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
  delete process.env['NO_TICKETS_AUTH_TIMEOUT_MS'];

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

function stubAuthServer(result: { token?: string; email?: string; rejectWith?: Error }): ReturnType<typeof vi.fn> {
  const close = vi.fn().mockResolvedValue(undefined);
  vi.mocked(authServer.startAuthServer).mockResolvedValue({
    port: 54321,
    callbackPromise: result.rejectWith
      ? Promise.reject(result.rejectWith)
      : Promise.resolve({
          token: result.token ?? 'nt_session_new',
          email: result.email ?? 'real@user.com',
        }),
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
    expect(opened.origin + opened.pathname).toBe('https://app.no-tickets.com/api/auth/cli');
    expect(opened.searchParams.get('port')).toBe('54321');
    expect(opened.searchParams.get('callback_port')).toBeNull();
    expect(opened.searchParams.get('code')).toMatch(/^[0-9a-f]{32}$/);

    const stored = loadCredentials();
    expect(stored?.token).toBe('nt_session_fresh');
    expect(process.exitCode).not.toBe(1);
    expect(logSpy).toHaveBeenCalledWith('Authenticated. Credentials saved to ~/.notickets/credentials.');
  });

  it('prints the URL before attempting to open the browser', async () => {
    stubAuthServer({ token: 'nt_session_fresh' });

    await runCli(['init'], { openBrowser });

    const urlHint = logSpy.mock.calls.find((call: unknown[]) =>
      typeof call[0] === 'string' && (call[0] as string).includes('app.no-tickets.com/api/auth/cli'),
    );
    expect(urlHint).toBeDefined();
  });

  it('honours NO_TICKETS_AUTH_URL override', async () => {
    vi.stubEnv('NO_TICKETS_AUTH_URL', 'https://app-staging.no-tickets.com/api/auth/cli');
    stubAuthServer({ token: 'nt_session_staging' });

    await runCli(['init'], { openBrowser });

    const opened = new URL(openedUrls[0]!);
    expect(opened.origin + opened.pathname).toBe('https://app-staging.no-tickets.com/api/auth/cli');
  });

  it('short-circuits when credentials already exist', async () => {
    saveCredentials('nt_session_existing', 'alice@example.com', '2099-01-01T00:00:00Z');

    await runCli(['init'], { openBrowser });

    expect(openedUrls).toHaveLength(0);
    expect(authServer.startAuthServer).not.toHaveBeenCalled();
    expect(logSpy).toHaveBeenCalledWith(expect.stringContaining('alice@example.com'));
  });

  it('saves the email returned by the auth server (no placeholder)', async () => {
    stubAuthServer({ token: 'nt_session_fresh', email: 'real@user.com' });

    await runCli(['init'], { openBrowser });

    expect(loadCredentials()?.email).toBe('real@user.com');
    const leaked = logSpy.mock.calls.find((call: unknown[]) =>
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

  it('forwards NO_TICKETS_AUTH_TIMEOUT_MS to the auth server as timeoutMs', async () => {
    vi.stubEnv('NO_TICKETS_AUTH_TIMEOUT_MS', '7500');
    stubAuthServer({ token: 'nt_session_t' });

    await runCli(['init'], { openBrowser });

    const startCall = vi.mocked(authServer.startAuthServer).mock.calls[0]?.[0];
    expect(startCall?.timeoutMs).toBe(7500);
  });

  it('forwards --timeout flag value to the auth server as timeoutMs', async () => {
    stubAuthServer({ token: 'nt_session_t' });

    await runCli(['init', '--timeout', '4500'], { openBrowser });

    const startCall = vi.mocked(authServer.startAuthServer).mock.calls[0]?.[0];
    expect(startCall?.timeoutMs).toBe(4500);
  });

  it('--timeout flag overrides NO_TICKETS_AUTH_TIMEOUT_MS', async () => {
    vi.stubEnv('NO_TICKETS_AUTH_TIMEOUT_MS', '7500');
    stubAuthServer({ token: 'nt_session_t' });

    await runCli(['init', '--timeout', '1000'], { openBrowser });

    const startCall = vi.mocked(authServer.startAuthServer).mock.calls[0]?.[0];
    expect(startCall?.timeoutMs).toBe(1000);
  });

  it('emits a periodic "still waiting" hint while the callback is pending', async () => {
    vi.useFakeTimers();
    try {
      let resolveCallback!: (v: { token: string; email: string }) => void;
      const callbackPromise = new Promise<{ token: string; email: string }>((resolve) => {
        resolveCallback = resolve;
      });
      vi.mocked(authServer.startAuthServer).mockResolvedValue({
        port: 54321,
        callbackPromise,
        close: vi.fn().mockResolvedValue(undefined),
      });

      const cliPromise = runCli(['init', '--timeout', '60000'], { openBrowser });

      // Drain microtasks so handleInit's setInterval is installed.
      await vi.advanceTimersByTimeAsync(0);
      // Cross the wait-hint interval boundary.
      await vi.advanceTimersByTimeAsync(10_000);

      const hint = logSpy.mock.calls.find((call: unknown[]) =>
        typeof call[0] === 'string' && (call[0] as string).toLowerCase().includes('still waiting'),
      );
      expect(hint).toBeDefined();

      // Let the auth flow complete so cliPromise settles cleanly.
      resolveCallback({ token: 'nt_session_done', email: 'a@b.com' });
      await vi.advanceTimersByTimeAsync(0);
      await cliPromise;
    } finally {
      vi.useRealTimers();
    }
  });

  it('SIGINT during init closes the auth server and prints "Cancelled."', async () => {
    let rejectCallback!: (err: Error) => void;
    const callbackPromise = new Promise<{ token: string; email: string }>((_, reject) => {
      rejectCallback = reject;
    });
    // Real auth-server contract: close() rejects callbackPromise with
    // "Auth server closed". The mock has to honour that or the CLI hangs.
    const close = vi.fn(async () => {
      rejectCallback(new Error('Auth server closed'));
    });
    vi.mocked(authServer.startAuthServer).mockResolvedValue({
      port: 54321,
      callbackPromise,
      close,
    });

    const cliPromise = runCli(['init'], { openBrowser });

    // Wait for handleInit to install its SIGINT listener.
    await new Promise((resolve) => setImmediate(resolve));

    process.emit('SIGINT');

    await cliPromise;

    expect(close).toHaveBeenCalled();
    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining('Cancelled'));
    expect(process.exitCode).toBe(130);
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
