import { createRequire } from 'node:module';
import { assemblePush, mergeSession } from './commands/push.js';
import { detectAgent } from './agent-detect.js';
import { createApiClient } from './sdk/api-client.js';
import { buildPushAuth } from './commands/push-auth.js';
import { pushSchema } from './core/schemas.js';
import { validateFiles } from './commands/validate.js';
import { readNoTicketsDir } from './core/fs.js';
import { spawn } from 'node:child_process';
import { describeAuthStatus, resolveAuth, DEFAULT_API_URL, NOT_AUTHENTICATED_MESSAGE } from './sdk/auth.js';
import { createToken, listTokens, revokeToken } from './commands/token.js';
import { resolveInitAuth } from './commands/init-auth.js';
import { DEFAULT_TIMEOUT_MS as DEFAULT_AUTH_TIMEOUT_MS } from './sdk/auth-server.js';

const DEFAULT_AUTH_URL = 'https://app.no-tickets.com/api/auth/cli';

export interface CliDeps {
  /** Override for the browser opener. Tests inject a stub; production uses platformBrowserOpener. */
  readonly openBrowser?: (url: string) => Promise<void>;
}

const require = createRequire(import.meta.url);
const { version: CLI_VERSION } = require('../package.json') as { version: string };

type Command = 'push' | 'init' | 'connect' | 'disconnect' | 'status' | 'validate' | 'token' | 'help' | 'version' | 'unknown';

const KNOWN_COMMANDS = new Set<Command>([
  'push', 'init', 'connect', 'disconnect', 'status', 'validate', 'token',
]);

function isKnownCommand(value: string): value is Command {
  return KNOWN_COMMANDS.has(value as Command);
}

type FlagValue = boolean | string;

/** Flags that consume the following argv entry as their value.
 *  All other flags are parsed as booleans so positional args like
 *  `push --dry-run some-file` are never accidentally swallowed. */
const VALUE_FLAGS = new Set<string>(['project', 'label', 'timeout']);

interface ParsedArgs {
  readonly command: Command;
  readonly args: readonly string[];
  readonly flags: Readonly<Record<string, FlagValue>>;
}

async function readStdin(): Promise<string> {
  const chunks: Buffer[] = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk as Buffer);
  }
  return Buffer.concat(chunks).toString('utf-8');
}

function loadPushConfig() {
  const apiUrl = process.env['NO_TICKETS_API_URL'] ?? DEFAULT_API_URL;
  const teamId = process.env['NO_TICKETS_TEAM_ID'] ?? '';
  const projectId = process.env['NO_TICKETS_PROJECT_ID'] ?? '';
  return { apiUrl, teamId, projectId };
}

async function handlePush(flags: Readonly<Record<string, FlagValue>>): Promise<void> {
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

function fail(message: string): void {
  console.error(message);
  process.exitCode = 1;
}

function requireSessionToken(): string | null {
  try {
    return resolveAuth().token;
  } catch {
    fail(NOT_AUTHENTICATED_MESSAGE);
    return null;
  }
}

function flagString(flags: Readonly<Record<string, FlagValue>>, key: string): string | undefined {
  const value = flags[key];
  return typeof value === 'string' ? value : undefined;
}

function handleRequestResult(result: { readonly success: boolean; readonly error?: string }, onSuccess: () => void): void {
  if (!result.success) {
    fail(result.error ?? 'Request failed');
    return;
  }
  onSuccess();
}

async function handleToken(
  subcommandArgs: readonly string[],
  flags: Readonly<Record<string, FlagValue>>,
): Promise<void> {
  const subcommand = subcommandArgs[0];
  const apiUrl = process.env['NO_TICKETS_API_URL'] ?? DEFAULT_API_URL;

  switch (subcommand) {
    case 'list': {
      const sessionToken = requireSessionToken();
      if (sessionToken === null) return;
      const result = await listTokens({ apiUrl, sessionToken });
      handleRequestResult(result, () => console.log(JSON.stringify({ tokens: result.tokens })));
      return;
    }
    case 'create': {
      const projectId = flagString(flags, 'project');
      const label = flagString(flags, 'label');
      if (!projectId) return fail('token create: --project <projectId> is required');
      if (!label) return fail('token create: --label <label> is required');
      const sessionToken = requireSessionToken();
      if (sessionToken === null) return;
      const result = await createToken({ apiUrl, sessionToken, projectId, label });
      handleRequestResult(result, () => console.log(JSON.stringify({ id: result.id, token: result.token })));
      return;
    }
    case 'revoke': {
      const tokenId = subcommandArgs[1];
      if (!tokenId) return fail('token revoke: <tokenId> is required');
      const sessionToken = requireSessionToken();
      if (sessionToken === null) return;
      const result = await revokeToken({ apiUrl, sessionToken, tokenId });
      handleRequestResult(result, () => console.log(JSON.stringify({ success: true })));
      return;
    }
    default:
      fail(`Unknown token subcommand: ${subcommand ?? '(none)'}. Use list | create | revoke.`);
  }
}

export function platformBrowserOpener(url: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const platform = process.platform;
    const [cmd, ...args] =
      platform === 'darwin' ? ['open', url] :
      platform === 'win32' ? ['cmd', '/c', 'start', '', url] :
      ['xdg-open', url];
    // windowsVerbatimArguments keeps the empty "" title and URL argv intact
    // so Windows shell metacharacters in the URL are not re-interpreted.
    const child = spawn(cmd!, args, {
      stdio: 'ignore',
      detached: true,
      windowsVerbatimArguments: platform === 'win32',
    });
    child.on('error', reject);
    child.on('spawn', () => {
      child.unref();
      resolve();
    });
  });
}

const WAIT_HINT_INTERVAL_MS = 10_000;

/** Parse a positive-integer ms value. Returns null on invalid input so the
 *  caller can decide between hard-fail (explicit user input) and silent
 *  fallback (env var / unset). */
function parsePositiveMs(raw: string | undefined): number | null {
  if (raw === undefined) return null;
  const parsed = Number(raw);
  if (!Number.isFinite(parsed) || parsed <= 0) return null;
  return parsed;
}

function resolveAuthTimeout(flags: Readonly<Record<string, FlagValue>>): number | { error: string } {
  const flagRaw = flagString(flags, 'timeout');
  if (flagRaw !== undefined) {
    const parsed = parsePositiveMs(flagRaw);
    if (parsed === null) {
      return { error: `--timeout: ${flagRaw} is not a positive number of milliseconds` };
    }
    return parsed;
  }
  const envParsed = parsePositiveMs(process.env['NO_TICKETS_AUTH_TIMEOUT_MS']);
  return envParsed ?? DEFAULT_AUTH_TIMEOUT_MS;
}

async function handleInit(
  openBrowser: (url: string) => Promise<void>,
  flags: Readonly<Record<string, FlagValue>>,
): Promise<void> {
  const authUrl = process.env['NO_TICKETS_AUTH_URL'] ?? DEFAULT_AUTH_URL;
  const timeoutResult = resolveAuthTimeout(flags);
  if (typeof timeoutResult === 'object') return fail(timeoutResult.error);
  const timeoutMs = timeoutResult;

  let waitHintTimer: NodeJS.Timeout | undefined;
  let sigintHandler: (() => void) | undefined;
  let closeFiredBySigint = false;
  let completed = false;

  const cleanup = (): void => {
    if (waitHintTimer) clearInterval(waitHintTimer);
    if (sigintHandler) {
      process.off('SIGINT', sigintHandler);
      sigintHandler = undefined;
    }
  };

  try {
    const result = await resolveInitAuth({
      authUrl,
      timeoutMs,
      onServerReady: ({ close }) => {
        sigintHandler = () => {
          // Race guard: if the auth flow already completed, ignore SIGINT.
          if (completed) return;
          closeFiredBySigint = true;
          close().catch(() => { /* close errors are non-actionable */ });
        };
        process.once('SIGINT', sigintHandler);

        // Skip the periodic hint when the timeout is shorter than one interval —
        // there's no value in a single "waiting (10s / 5s)…" line.
        if (timeoutMs >= WAIT_HINT_INTERVAL_MS) {
          const startedAt = Date.now();
          const totalSeconds = Math.round(timeoutMs / 1000);
          waitHintTimer = setInterval(() => {
            const elapsed = Math.round((Date.now() - startedAt) / 1000);
            console.log(`Still waiting for browser callback (${elapsed}s / ${totalSeconds}s)…`);
          }, WAIT_HINT_INTERVAL_MS);
          // unref so a stuck wait timer can never block process exit.
          waitHintTimer.unref();
        }
      },
      openBrowser: async (url) => {
        console.log(`Opening browser to authenticate:\n  ${url}\n(If the browser does not open, paste the URL above.)`);
        try {
          await openBrowser(url);
        } catch {
          // Non-fatal — the URL is already printed for manual paste.
        }
      },
    });
    completed = true;
    cleanup();
    if (result.isNewAuth) {
      console.log('Authenticated. Credentials saved to ~/.notickets/credentials.');
    } else {
      console.log(`Already authenticated as ${result.email}. Run \`rm ~/.notickets/credentials\` to sign out.`);
    }
  } catch (err) {
    cleanup();
    if (closeFiredBySigint) {
      console.error('Cancelled.');
      process.exitCode = 130;
      return;
    }
    fail(err instanceof Error ? err.message : 'Authentication failed');
  }
}

function handleStatus(): void {
  try {
    console.log(JSON.stringify(describeAuthStatus()));
  } catch {
    console.error(NOT_AUTHENTICATED_MESSAGE);
    process.exitCode = 1;
  }
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
export async function runCli(argv: readonly string[], deps: CliDeps = {}): Promise<void> {
  const parsed = parseArgs(argv);
  const openBrowser = deps.openBrowser ?? platformBrowserOpener;

  switch (parsed.command) {
    case 'help':
      console.log('Usage: npx no-tickets <command> [options]\n\nCommands: init, push, status, validate, connect, disconnect, token');
      break;
    case 'version':
      console.log(CLI_VERSION);
      break;
    case 'push':
      await handlePush(parsed.flags);
      break;
    case 'validate':
      await handleValidate();
      break;
    case 'status':
      handleStatus();
      break;
    case 'token':
      await handleToken(parsed.args, parsed.flags);
      break;
    case 'init':
      await handleInit(openBrowser, parsed.flags);
      break;
    case 'unknown':
      console.error(`Unknown command: ${String(argv[0]).replace(/[\x00-\x1f\x7f]/g, '')}\nRun "npx no-tickets --help" for usage.`);
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
  const flags: Record<string, FlagValue> = {};

  for (let i = command === 'unknown' ? 0 : 1; i < argv.length; i++) {
    const arg = argv[i];
    if (!arg) continue;

    if (arg.startsWith('--')) {
      const key = arg.slice(2);
      if (VALUE_FLAGS.has(key)) {
        const next = argv[i + 1];
        if (next !== undefined && next !== '' && !next.startsWith('--')) {
          flags[key] = next;
          i++;
          continue;
        }
      }
      flags[key] = true;
    } else {
      args.push(arg);
    }
  }

  return { command, args, flags };
}
