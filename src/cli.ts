import { readdir, readFile, stat } from 'node:fs/promises';
import { join } from 'node:path';
import { assemblePush, mergeSession } from './commands/push.js';
import { detectAgent } from './agent-detect.js';
import { createApiClient } from './sdk/api-client.js';
import { buildPushAuth } from './commands/push-auth.js';
import { pushSchema } from './core/schemas.js';
import { validateFiles } from './commands/validate.js';
import type { FileEntry } from './core/types.js';

type Command = 'push' | 'init' | 'connect' | 'disconnect' | 'status' | 'validate' | 'help' | 'version' | 'unknown';

const KNOWN_COMMANDS = new Set<Command>([
  'push', 'init', 'connect', 'disconnect', 'status', 'validate',
]);

function isKnownCommand(value: string): value is Command {
  return KNOWN_COMMANDS.has(value as Command);
}

interface ParsedArgs {
  readonly command: Command;
  readonly args: readonly string[];
  readonly flags: Readonly<Record<string, boolean>>;
}

async function readNoTicketsDir(dir: string): Promise<readonly FileEntry[]> {
  const entries: FileEntry[] = [];
  let items: string[];
  try {
    items = await readdir(dir);
  } catch {
    return [];
  }

  for (const item of items) {
    const itemPath = join(dir, item);
    const itemStat = await stat(itemPath).catch(() => null);
    if (itemStat === null) continue;

    if (itemStat.isFile() && item.endsWith('.md')) {
      const content = await readFile(itemPath, 'utf-8');
      entries.push({ path: itemPath, content });
    } else if (itemStat.isDirectory()) {
      const subItems = await readdir(itemPath).catch(() => [] as string[]);
      for (const subItem of subItems) {
        if (!subItem.endsWith('.md')) continue;
        const subPath = join(itemPath, subItem);
        const content = await readFile(subPath, 'utf-8').catch(() => null);
        if (content !== null) {
          entries.push({ path: subPath, content });
        }
      }
    }
  }

  return entries;
}

async function readStdin(): Promise<string> {
  const chunks: Buffer[] = [];
  for await (const chunk of process.stdin) {
    chunks.push(Buffer.from(chunk as Uint8Array));
  }
  return Buffer.concat(chunks).toString('utf-8');
}

function loadPushConfig() {
  const apiUrl = process.env['NO_TICKETS_API_URL'] ?? 'https://api.no-tickets.com';
  const teamId = process.env['NO_TICKETS_TEAM_ID'] ?? '';
  const projectId = process.env['NO_TICKETS_PROJECT_ID'] ?? '';
  return { apiUrl, teamId, projectId };
}

async function handlePush(flags: Readonly<Record<string, boolean>>): Promise<void> {
  const session = detectAgent();
  const isStdin = Boolean(flags['stdin']);
  const isDryRun = Boolean(flags['dry-run']);
  const config = loadPushConfig();

  const payload = isStdin
    ? await buildStdinPush(session)
    : await buildFilePush(config.projectId, session);

  if (isDryRun) {
    console.log(JSON.stringify(payload, null, 2));
    return;
  }

  const authConfig = buildPushAuth(config);
  const client = createApiClient({ token: authConfig.token, apiUrl: authConfig.apiUrl });
  const result = await client.push(payload);
  console.log(JSON.stringify(result));
}

async function buildStdinPush(session: ReturnType<typeof detectAgent>) {
  const raw = await readStdin();
  const parsed = pushSchema.parse(JSON.parse(raw));
  return mergeSession(parsed, session);
}

async function handleValidate(): Promise<void> {
  const files = await readNoTicketsDir('.notickets');
  const result = validateFiles(files);

  if (result.valid) {
    console.log('Validation passed — no errors found.');
    return;
  }

  for (const error of result.errors) {
    const location = error.field ? `${error.file}:${error.field}` : error.file;
    console.error(`ERROR ${location}: ${error.message}`);
    if (error.suggestion) {
      console.error(`  suggestion: ${error.suggestion}`);
    }
  }

  process.exitCode = 1;
}

async function buildFilePush(projectId: string, session: ReturnType<typeof detectAgent>) {
  if (!projectId) {
    throw new Error('NO_TICKETS_PROJECT_ID environment variable is required for push');
  }
  const files = await readNoTicketsDir('.notickets');
  const payload = assemblePush({ files, projectId, session });
  pushSchema.parse(payload);
  return payload;
}

/**
 * Run the CLI with the given arguments.
 */
export async function runCli(argv: readonly string[]): Promise<void> {
  const parsed = parseArgs(argv);

  switch (parsed.command) {
    case 'help':
      console.log('Usage: npx no-tickets <command> [options]\n\nCommands: init, push, status, validate, connect, disconnect');
      break;
    case 'version':
      console.log('2.0.0');
      break;
    case 'push':
      await handlePush(parsed.flags);
      break;
    case 'validate':
      await handleValidate();
      break;
    case 'init':
      console.error('Command "init" is not yet implemented.');
      process.exitCode = 1;
      break;
    case 'unknown':
      console.error(`Unknown command: ${argv[0]}\nRun "npx no-tickets --help" for usage.`);
      process.exitCode = 1;
      break;
    default:
      console.error(`Command "${parsed.command}" is not yet implemented.`);
      process.exitCode = 1;
      break;
  }
}

/**
 * Parse CLI arguments into command, positional args, and flags.
 * Pure function — no I/O.
 */
export function parseArgs(argv: readonly string[]): ParsedArgs {
  if (argv.length === 0) {
    return { command: 'help', args: [], flags: {} };
  }

  const first = argv[0];

  if (first === '--help' || first === '-h') {
    return { command: 'help', args: [], flags: {} };
  }

  if (first === '--version' || first === '-v') {
    return { command: 'version', args: [], flags: {} };
  }

  const command: Command = isKnownCommand(first ?? '') ? first as Command : 'unknown';
  const args: string[] = [];
  const flags: Record<string, boolean> = {};

  for (let i = command === 'unknown' ? 0 : 1; i < argv.length; i++) {
    const arg = argv[i];
    if (!arg) continue;

    if (arg.startsWith('--')) {
      flags[arg.slice(2)] = true;
    } else {
      args.push(arg);
    }
  }

  return { command, args, flags };
}
