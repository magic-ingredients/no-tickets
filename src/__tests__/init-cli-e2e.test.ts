import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runCli } from '../cli.js';
import { loadCredentials, saveCredentials } from '../sdk/credentials.js';
import * as authServer from '../sdk/auth-server.js';

let testDir: string;
let logSpy: ReturnType<typeof vi.spyOn>;
let errSpy: ReturnType<typeof vi.spyOn>;

vi.mock('../sdk/auth-server.js');

let openedUrls: string[];
const openBrowser = async (url: string): Promise<void> => {
  openedUrls.push(url);
};

beforeEach(async () => {
  vi.clearAllMocks();
  testDir = await mkdtemp(join(tmpdir(), 'nt-init-cli-e2e-'));
  vi.stubEnv('NO_TICKETS_HOME', testDir);
  delete process.env['NO_TICKETS_TOKEN'];
  delete process.env['NO_TICKETS_AUTH_URL'];

  openedUrls = [];
  logSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
  errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  process.exitCode = undefined;
});

afterEach(async () => {
  vi.unstubAllEnvs();
  vi.restoreAllMocks();
  await rm(testDir, { recursive: true, force: true });
});

function stubAuthServer(result: { token?: string; rejectWith?: Error }): ReturnType<typeof vi.fn> {
  const close = vi.fn().mockResolvedValue(undefined);
  vi.mocked(authServer.startAuthServer).mockResolvedValue({
    port: 54321,
    tokenPromise: result.rejectWith
      ? Promise.reject(result.rejectWith)
      : Promise.resolve(result.token ?? 'nt_session_new'),
    close,
  });
  return close;
}

describe('init command e2e', () => {
  it('opens the browser at the default auth URL with a callback_port and saves credentials on success', async () => {
    stubAuthServer({ token: 'nt_session_fresh' });

    await runCli(['init'], { openBrowser });

    expect(openedUrls).toHaveLength(1);
    const opened = new URL(openedUrls[0]!);
    expect(opened.origin + opened.pathname).toBe('https://app.no-tickets.com/auth/cli');
    expect(opened.searchParams.get('callback_port')).toBe('54321');

    const stored = loadCredentials();
    expect(stored?.token).toBe('nt_session_fresh');
    expect(process.exitCode).not.toBe(1);
  });

  it('prints the URL before attempting to open the browser', async () => {
    stubAuthServer({ token: 'nt_session_fresh' });

    await runCli(['init'], { openBrowser });

    const urlHint = logSpy.mock.calls.find((call) =>
      typeof call[0] === 'string' && (call[0] as string).includes('app.no-tickets.com/auth/cli'),
    );
    expect(urlHint).toBeDefined();
  });

  it('honours NO_TICKETS_AUTH_URL override', async () => {
    vi.stubEnv('NO_TICKETS_AUTH_URL', 'https://app-staging.no-tickets.com/auth/cli');
    stubAuthServer({ token: 'nt_session_staging' });

    await runCli(['init'], { openBrowser });

    const opened = new URL(openedUrls[0]!);
    expect(opened.origin + opened.pathname).toBe('https://app-staging.no-tickets.com/auth/cli');
  });

  it('short-circuits when credentials already exist', async () => {
    saveCredentials('nt_session_existing', 'alice@example.com', '2099-01-01T00:00:00Z');

    await runCli(['init'], { openBrowser });

    expect(openedUrls).toHaveLength(0);
    expect(authServer.startAuthServer).not.toHaveBeenCalled();
    expect(logSpy).toHaveBeenCalledWith(expect.stringContaining('alice@example.com'));
  });

  it('does NOT print the placeholder email on a new auth', async () => {
    stubAuthServer({ token: 'nt_session_fresh' });

    await runCli(['init'], { openBrowser });

    // resolveInitAuth saves a hardcoded 'authenticated@no-tickets.com' placeholder
    // pending server-side change. The CLI must not surface it.
    const leaked = logSpy.mock.calls.find((call) =>
      typeof call[0] === 'string' && (call[0] as string).includes('authenticated@no-tickets.com'),
    );
    expect(leaked).toBeUndefined();
  });

  it('exits 1 and surfaces the auth-server error message', async () => {
    const SENTINEL = 'init-test-sentinel-error-xyz';
    stubAuthServer({ rejectWith: new Error(SENTINEL) });

    await runCli(['init'], { openBrowser });

    expect(process.exitCode).toBe(1);
    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining(SENTINEL));
  });

  it('still completes auth when the browser opener rejects (URL-paste fallback)', async () => {
    stubAuthServer({ token: 'nt_session_manual' });
    const brokenOpener = vi.fn().mockRejectedValue(new Error('xdg-open: not found'));

    await runCli(['init'], { openBrowser: brokenOpener });

    expect(brokenOpener).toHaveBeenCalledOnce();
    expect(loadCredentials()?.token).toBe('nt_session_manual');
    expect(process.exitCode).not.toBe(1);
  });
});
