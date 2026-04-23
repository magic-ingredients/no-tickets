import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runCli } from '../cli.js';

let testDir: string;
let logSpy: ReturnType<typeof vi.spyOn>;
let errSpy: ReturnType<typeof vi.spyOn>;

beforeEach(async () => {
  testDir = await mkdtemp(join(tmpdir(), 'nt-status-e2e-'));
  vi.stubEnv('HOME', testDir);
  vi.stubEnv('NO_TICKETS_TOKEN', '');
  delete process.env['NO_TICKETS_TOKEN'];
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

    await runCli(['status']);

    expect(logSpy).toHaveBeenCalledOnce();
    const output = JSON.parse(logSpy.mock.calls[0]![0] as string);
    expect(output.authenticated).toBe(true);
    expect(output.source).toBe('env');
    expect(output.tokenType).toBe('push');
    expect(output.apiUrl).toBe('https://api.test.com');
  });

  it('reports not authenticated when no credentials present', async () => {
    await runCli(['status']);

    expect(errSpy).toHaveBeenCalled();
    expect(process.exitCode).toBe(1);
  });

  it('defaults apiUrl when NO_TICKETS_API_URL unset', async () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_session_abc');
    vi.stubEnv('NO_TICKETS_API_URL', '');
    delete process.env['NO_TICKETS_API_URL'];

    await runCli(['status']);

    const output = JSON.parse(logSpy.mock.calls[0]![0] as string);
    expect(output.apiUrl).toBe('https://api.no-tickets.com');
    expect(output.tokenType).toBe('session');
  });
});
