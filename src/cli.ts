import { readdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { assemblePush, mergeSession } from './commands/push.js';
import { detectAgent } from './agent-detect.js';
import { createApiClient } from './sdk/api-client.js';
import { buildPushAuth, type PushLocalConfig } from './commands/push-auth.js';
import { pushSchema } from './core/schemas.js';
import type { FileEntry, Push } from './core/types.js';

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
    const stat = await readFile(itemPath, 'utf-8').catch(() => null);
    if (stat !== null && item.endsWith('.md')) {
      entries.push({ path: itemPath, content: stat });
      continue;
    }
    // Recurse into subdirectories
    let subItems: string[];
    try {
      subItems = await readdir(itemPath);
    } catch {
      continue;
    }
    for (const subItem of subItems) {
      if (!subItem.endsWith('.md')) continue;
      const subPath = join(itemPath, subItem);
      const content = await readFile(subPath, 'utf-8').catch(() => null);
      if (content !== null) {
        entries.push({ path: subPath, content });
      }
    }
  }

  return entries;
}

async function readStdin(): Promise<string> {
  const chunks: Buffer[] = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk as Buffer);
  }
  return Buffer.concat(chunks).toString('utf-8');
}

async function handlePush(flags: Readonly<Record<string, boolean>>): Promise<void> {
  const session = detectAgent();
  const isStdin = Boolean(flags['stdin']);
  const isDryRun = Boolean(flags['dry-run']);

  let payload: Push;

  if (isStdin) {
    const raw = await readStdin();
    const parsed = JSON.parse(raw) as Push;
    const validated = pushSchema.parse(parsed);
    payload = mergeSession(validated as Push, session);
  } else {
    const files = await readNoTicketsDir('.notickets');
    const localConfig: PushLocalConfig = {
      apiUrl: process.env['NO_TICKETS_API_URL'] ?? 'https://api.no-tickets.com',
      teamId: process.env['NO_TICKETS_TEAM_ID'] ?? '',
      projectId: process.env['NO_TICKETS_PROJECT_ID'] ?? '',
    };
    payload = assemblePush({
      files,
      projectId: localConfig.projectId,
      session,
    });
    pushSchema.parse(payload);
  }

  if (isDryRun) {
    console.log(JSON.stringify(payload, null, 2));
    return;
  }

  const authConfig = buildPushAuth({
    apiUrl: process.env['NO_TICKETS_API_URL'] ?? 'https://api.no-tickets.com',
    teamId: process.env['NO_TICKETS_TEAM_ID'] ?? '',
    projectId: process.env['NO_TICKETS_PROJECT_ID'] ?? '',
  });
  const client = createApiClient({ token: authConfig.token, apiUrl: authConfig.apiUrl });
  const result = await client.push(payload);
  console.log(JSON.stringify(result));
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
