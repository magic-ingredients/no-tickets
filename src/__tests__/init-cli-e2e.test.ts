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
  delete process.env['NO_TICKETS_API_URL'];
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

  it('honours NO_TICKETS_AUTH_URL + NO_TICKETS_API_URL pair override', async () => {
    vi.stubEnv('NO_TICKETS_API_URL', 'https://api-staging.no-tickets.com');
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

  it('--profile loads URLs from ~/.notickets/config.json', async () => {
    const { mkdir, writeFile } = await import('node:fs/promises');
    await mkdir(join(testDir, '.notickets'), { recursive: true });
    await writeFile(
      join(testDir, '.notickets', 'config.json'),
      JSON.stringify({
        profiles: {
          staging: {
            apiUrl: 'https://api-staging.example.com',
            authUrl: 'https://app-staging.example.com/api/auth/cli',
          },
        },
      }),
    );
    stubAuthServer({ token: 'nt_session_p' });

    await runCli(['init', '--profile', 'staging'], { openBrowser });

    const opened = new URL(openedUrls[0]!);
    expect(opened.origin + opened.pathname).toBe('https://app-staging.example.com/api/auth/cli');
  });

  it('exits 1 with a helpful error when --profile is set but config file does not exist', async () => {
    await runCli(['init', '--profile', 'staging'], { openBrowser });

    expect(process.exitCode).toBe(1);
    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining('does not exist'));
    expect(authServer.startAuthServer).not.toHaveBeenCalled();
  });

  it('exits 1 when only NO_TICKETS_API_URL is set (pair validation)', async () => {
    vi.stubEnv('NO_TICKETS_API_URL', 'https://api.example.com');

    await runCli(['init'], { openBrowser });

    expect(process.exitCode).toBe(1);
    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining('NO_TICKETS_AUTH_URL'));
    expect(authServer.startAuthServer).not.toHaveBeenCalled();
  });

  it('echoes the resolved API and Auth URLs before opening the browser', async () => {
    stubAuthServer({ token: 'nt_session_x' });

    await runCli(['init'], { openBrowser });

    expect(logSpy).toHaveBeenCalledWith('Using API: https://api.no-tickets.com');
    expect(logSpy).toHaveBeenCalledWith('Using Auth: https://app.no-tickets.com/api/auth/cli');
  });

  it('echoes the resolved env-var URLs (not the defaults) when both env vars are set', async () => {
    vi.stubEnv('NO_TICKETS_API_URL', 'https://api-staging.example.com');
    vi.stubEnv('NO_TICKETS_AUTH_URL', 'https://app-staging.example.com/api/auth/cli');
    stubAuthServer({ token: 'nt_session_x' });

    await runCli(['init'], { openBrowser });

    expect(logSpy).toHaveBeenCalledWith('Using API: https://api-staging.example.com');
    expect(logSpy).toHaveBeenCalledWith('Using Auth: https://app-staging.example.com/api/auth/cli');
  });

  it('exits 1 when only NO_TICKETS_AUTH_URL is set (inverse pair-validation)', async () => {
    vi.stubEnv('NO_TICKETS_AUTH_URL', 'https://app.example.com/api/auth/cli');

    await runCli(['init'], { openBrowser });

    expect(process.exitCode).toBe(1);
    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining('NO_TICKETS_API_URL'));
    expect(authServer.startAuthServer).not.toHaveBeenCalled();
  });

  it('warns when --profile shadows NO_TICKETS_API_URL / NO_TICKETS_AUTH_URL env vars', async () => {
    const { mkdir, writeFile } = await import('node:fs/promises');
    await mkdir(join(testDir, '.notickets'), { recursive: true });
    await writeFile(
      join(testDir, '.notickets', 'config.json'),
      JSON.stringify({
        profiles: {
          staging: { apiUrl: 'https://from-profile.example.com', authUrl: 'https://from-profile-auth.example.com/api/auth/cli' },
        },
      }),
    );
    vi.stubEnv('NO_TICKETS_API_URL', 'https://from-env.example.com');
    vi.stubEnv('NO_TICKETS_AUTH_URL', 'https://from-env-auth.example.com/api/auth/cli');
    stubAuthServer({ token: 'nt_session_x' });

    await runCli(['init', '--profile', 'staging'], { openBrowser });

    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining('shadowing'));
    // And the profile URL still wins.
    expect(logSpy).toHaveBeenCalledWith('Using API: https://from-profile.example.com');
  });

  it('exits 1 with a clear error when --timeout is not a positive number', async () => {
    await runCli(['init', '--timeout', 'abc'], { openBrowser });

    expect(process.exitCode).toBe(1);
    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining('--timeout'));
    expect(authServer.startAuthServer).not.toHaveBeenCalled();
  });

  it('exits 1 when --timeout is zero', async () => {
    await runCli(['init', '--timeout', '0'], { openBrowser });

    expect(process.exitCode).toBe(1);
    expect(authServer.startAuthServer).not.toHaveBeenCalled();
  });

  it('falls back to default when NO_TICKETS_AUTH_TIMEOUT_MS is malformed (no flag set)', async () => {
    vi.stubEnv('NO_TICKETS_AUTH_TIMEOUT_MS', 'not-a-number');
    stubAuthServer({ token: 'nt_session_t' });

    await runCli(['init'], { openBrowser });

    const startCall = vi.mocked(authServer.startAuthServer).mock.calls[0]?.[0];
    expect(startCall?.timeoutMs).toBe(120_000);
  });

  it('skips the periodic wait hint when --timeout is shorter than one interval', async () => {
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

      const cliPromise = runCli(['init', '--timeout', '5000'], { openBrowser });
      await vi.advanceTimersByTimeAsync(0);
      await vi.advanceTimersByTimeAsync(15_000);

      const hint = logSpy.mock.calls.find((call: unknown[]) =>
        typeof call[0] === 'string' && (call[0] as string).toLowerCase().includes('still waiting'),
      );
      expect(hint).toBeUndefined();

      resolveCallback({ token: 'nt_session_done', email: 'a@b.com' });
      await vi.advanceTimersByTimeAsync(0);
      await cliPromise;
    } finally {
      vi.useRealTimers();
    }
  });

  it('SIGINT handler captured before completion is a no-op once `completed=true`', async () => {
    // We can't drive this through process.emit('SIGINT') because process.once
    // removes the listener after one fire, so by the time auth has completed
    // there's nothing for emit() to invoke. Instead capture the actual handler
    // function passed to process.once and call it directly post-completion.
    const onceSpy = vi.spyOn(process, 'once');
    const close = vi.fn().mockResolvedValue(undefined);
    vi.mocked(authServer.startAuthServer).mockResolvedValue({
      port: 54321,
      callbackPromise: Promise.resolve({ token: 'nt_session_done', email: 'a@b.com' }),
      close,
    });

    await runCli(['init'], { openBrowser });

    const sigintRegistration = onceSpy.mock.calls.find((call) => call[0] === 'SIGINT');
    expect(sigintRegistration).toBeDefined();
    const handler = sigintRegistration![1] as () => void;

    // resolveInitAuth's finally block already calls close() on success — that
    // call doesn't count for this assertion. Reset before the simulated late
    // SIGINT so we can isolate whether the handler triggered an extra close.
    close.mockClear();

    handler();
    await new Promise((resolve) => setImmediate(resolve));

    expect(close).not.toHaveBeenCalled();
  });

  it('cleanup() removes the SIGINT listener and clears the wait timer', async () => {
    const onceSpy = vi.spyOn(process, 'once');
    const offSpy = vi.spyOn(process, 'off');
    stubAuthServer({ token: 'nt_session_t' });

    await runCli(['init', '--timeout', '60000'], { openBrowser });

    const sigintReg = onceSpy.mock.calls.find((call) => call[0] === 'SIGINT');
    const sigintRem = offSpy.mock.calls.find((call) => call[0] === 'SIGINT');
    expect(sigintReg).toBeDefined();
    expect(sigintRem).toBeDefined();
    expect(sigintReg![1]).toBe(sigintRem![1]);
  });

  it('wait hint fires exactly at the WAIT_HINT_INTERVAL_MS boundary and reports correct numbers', async () => {
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

      // --timeout exactly equal to WAIT_HINT_INTERVAL_MS — boundary case.
      const cliPromise = runCli(['init', '--timeout', '10000'], { openBrowser });
      await vi.advanceTimersByTimeAsync(0);
      await vi.advanceTimersByTimeAsync(10_000);

      // Pin the exact format including elapsed and total seconds.
      expect(logSpy).toHaveBeenCalledWith('Still waiting for browser callback (10s / 10s)…');

      resolveCallback({ token: 'nt_session_done', email: 'a@b.com' });
      await vi.advanceTimersByTimeAsync(0);
      await cliPromise;
    } finally {
      vi.useRealTimers();
    }
  });

  it('does not emit further wait hints after auth completes', async () => {
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
      await vi.advanceTimersByTimeAsync(0);
      await vi.advanceTimersByTimeAsync(10_000);
      const hintsAfterFirstInterval = logSpy.mock.calls.filter((call: unknown[]) =>
        typeof call[0] === 'string' && (call[0] as string).includes('Still waiting'),
      ).length;

      resolveCallback({ token: 'nt_session_done', email: 'a@b.com' });
      await vi.advanceTimersByTimeAsync(0);
      await cliPromise;

      // Crank the clock well past several would-be intervals.
      await vi.advanceTimersByTimeAsync(60_000);
      const hintsAfterCompletion = logSpy.mock.calls.filter((call: unknown[]) =>
        typeof call[0] === 'string' && (call[0] as string).includes('Still waiting'),
      ).length;

      expect(hintsAfterCompletion).toBe(hintsAfterFirstInterval);
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
