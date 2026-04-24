import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runCli } from '../cli.js';
import { saveCredentials } from '../sdk/credentials.js';

let testDir: string;
let logSpy: ReturnType<typeof vi.spyOn>;
let errSpy: ReturnType<typeof vi.spyOn>;

beforeEach(async () => {
  testDir = await mkdtemp(join(tmpdir(), 'nt-status-e2e-'));
  vi.stubEnv('NO_TICKETS_HOME', testDir);
  delete process.env['NO_TICKETS_TOKEN'];
  delete process.env['NO_TICKETS_API_URL'];
  delete process.env['NO_TICKETS_AUTH_URL'];
  logSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
  errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  process.exitCode = undefined;
});

afterEach(async () => {
  vi.unstubAllEnvs();
  vi.restoreAllMocks();
  await rm(testDir, { recursive: true, force: true });
});

describe('status command e2e', () => {
  it('reports authenticated state from NO_TICKETS_TOKEN env var', async () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_push_envtoken');
    vi.stubEnv('NO_TICKETS_API_URL', 'https://api.test.com');
    vi.stubEnv('NO_TICKETS_AUTH_URL', 'https://app.test.com/api/auth/cli');

    await runCli(['status']);

    expect(logSpy).toHaveBeenCalledOnce();
    const output = JSON.parse(logSpy.mock.calls[0]![0] as string);
    expect(output).toEqual({
      authenticated: true,
      source: 'env',
      tokenType: 'push',
      apiUrl: 'https://api.test.com',
      authUrl: 'https://app.test.com/api/auth/cli',
    });
  });

  it('reports credentials source when token is loaded from the credentials file', async () => {
    saveCredentials('nt_session_stored', 'alice@example.com', '2030-01-01T00:00:00Z');

    await runCli(['status']);

    const output = JSON.parse(logSpy.mock.calls[0]![0] as string);
    expect(output.source).toBe('credentials');
    expect(output.tokenType).toBe('session');
  });

  it('reports tokenType: unknown for tokens without a known prefix', async () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'abc123-not-a-real-prefix');

    await runCli(['status']);

    const output = JSON.parse(logSpy.mock.calls[0]![0] as string);
    expect(output.tokenType).toBe('unknown');
  });

  it('defaults apiUrl when NO_TICKETS_API_URL unset', async () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_session_abc');

    await runCli(['status']);

    const output = JSON.parse(logSpy.mock.calls[0]![0] as string);
    expect(output.apiUrl).toBe('https://api.no-tickets.com');
  });

  it('prints the documented not-authenticated message and exits 1 when no credentials present', async () => {
    await runCli(['status']);

    expect(errSpy).toHaveBeenCalledWith(
      'Not authenticated. Set NO_TICKETS_TOKEN or run `npx no-tickets init`.',
    );
    expect(process.exitCode).toBe(1);
  });
});
