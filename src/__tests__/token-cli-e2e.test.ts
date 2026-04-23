import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runCli } from '../cli.js';

let testDir: string;
let fetchSpy: ReturnType<typeof vi.fn>;
let logSpy: ReturnType<typeof vi.spyOn>;
let errSpy: ReturnType<typeof vi.spyOn>;

function jsonResponse(body: unknown, status = 200) {
  return Promise.resolve(new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  }));
}

beforeEach(async () => {
  testDir = await mkdtemp(join(tmpdir(), 'nt-token-cli-e2e-'));
  vi.stubEnv('NO_TICKETS_HOME', testDir);
  delete process.env['NO_TICKETS_TOKEN'];
  delete process.env['NO_TICKETS_API_URL'];

  fetchSpy = vi.fn();
  vi.stubGlobal('fetch', fetchSpy);

  logSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
  errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  process.exitCode = undefined;
});

afterEach(async () => {
  vi.unstubAllEnvs();
  vi.restoreAllMocks();
  await rm(testDir, { recursive: true, force: true });
});

describe('token list command e2e', () => {
  it('calls GET /v1/tokens with the session token and prints entries as JSON', async () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_session_secret');
    vi.stubEnv('NO_TICKETS_API_URL', 'https://api.test.com');
    fetchSpy.mockReturnValue(jsonResponse({
      tokens: [
        { id: 'tok-1', prefix: 'nt_push_ab', label: 'CI', createdAt: '2026-04-22T10:00:00Z' },
      ],
    }));

    await runCli(['token', 'list']);

    expect(fetchSpy).toHaveBeenCalledOnce();
    const [url, init] = fetchSpy.mock.calls[0] as [string, RequestInit];
    expect(url).toBe('https://api.test.com/v1/tokens');
    expect((init.headers as Record<string, string>)['Authorization']).toBe('Bearer nt_session_secret');

    const output = JSON.parse(logSpy.mock.calls[0]![0] as string);
    expect(output.tokens).toHaveLength(1);
    expect(output.tokens[0].id).toBe('tok-1');
  });

  it('exits 1 and prints error when API returns failure', async () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_session_secret');
    fetchSpy.mockReturnValue(jsonResponse({ error: 'Forbidden' }, 403));

    await runCli(['token', 'list']);

    expect(process.exitCode).toBe(1);
    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining('Forbidden'));
  });

  it('exits 1 when not authenticated', async () => {
    await runCli(['token', 'list']);

    expect(process.exitCode).toBe(1);
    expect(fetchSpy).not.toHaveBeenCalled();
  });
});

describe('token create command e2e', () => {
  it('calls POST /v1/tokens with projectId and label and prints the new token', async () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_session_secret');
    fetchSpy.mockReturnValue(jsonResponse({ id: 'tok-new', token: 'nt_push_new123' }));

    await runCli(['token', 'create', '--project', 'proj-1', '--label', 'CI push']);

    const [url, init] = fetchSpy.mock.calls[0] as [string, RequestInit];
    expect(url).toBe('https://api.no-tickets.com/v1/tokens');
    expect(init.method).toBe('POST');
    expect(JSON.parse(init.body as string)).toEqual({ projectId: 'proj-1', label: 'CI push' });

    const output = JSON.parse(logSpy.mock.calls[0]![0] as string);
    expect(output.id).toBe('tok-new');
    expect(output.token).toBe('nt_push_new123');
  });

  it('exits 1 when --project is missing', async () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_session_secret');

    await runCli(['token', 'create', '--label', 'CI']);

    expect(process.exitCode).toBe(1);
    expect(fetchSpy).not.toHaveBeenCalled();
    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining('--project'));
  });

  it('exits 1 when --label is missing', async () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_session_secret');

    await runCli(['token', 'create', '--project', 'p1']);

    expect(process.exitCode).toBe(1);
    expect(fetchSpy).not.toHaveBeenCalled();
    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining('--label'));
  });

  it('exits 1 when --project is present as boolean (no value consumed)', async () => {
    // `--project --label CI` parses --project as boolean true because the next
    // arg is another flag. flagString() must reject it rather than send
    // boolean garbage to the API.
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_session_secret');

    await runCli(['token', 'create', '--project', '--label', 'CI']);

    expect(process.exitCode).toBe(1);
    expect(fetchSpy).not.toHaveBeenCalled();
    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining('--project'));
  });

  it('exits 1 and reports the API error when the server returns a failure', async () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_session_secret');
    fetchSpy.mockReturnValue(jsonResponse({ error: 'project not found' }, 404));

    await runCli(['token', 'create', '--project', 'p1', '--label', 'CI']);

    expect(process.exitCode).toBe(1);
    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining('project not found'));
  });

  it('exits 1 when not authenticated', async () => {
    await runCli(['token', 'create', '--project', 'p1', '--label', 'CI']);

    expect(process.exitCode).toBe(1);
    expect(fetchSpy).not.toHaveBeenCalled();
    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining('Not authenticated'));
  });
});

describe('token revoke command e2e', () => {
  it('calls DELETE /v1/tokens/:id with the session token and prints { success: true }', async () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_session_secret');
    fetchSpy.mockReturnValue(jsonResponse({}));

    await runCli(['token', 'revoke', 'tok-1']);

    const [url, init] = fetchSpy.mock.calls[0] as [string, RequestInit];
    expect(url).toBe('https://api.no-tickets.com/v1/tokens/tok-1');
    expect(init.method).toBe('DELETE');
    expect((init.headers as Record<string, string>)['Authorization']).toBe('Bearer nt_session_secret');

    expect(logSpy).toHaveBeenCalledOnce();
    expect(JSON.parse(logSpy.mock.calls[0]![0] as string)).toEqual({ success: true });
  });

  it('url-encodes the token id', async () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_session_secret');
    fetchSpy.mockReturnValue(jsonResponse({}));

    await runCli(['token', 'revoke', 'tok/with/slashes']);

    const [url] = fetchSpy.mock.calls[0] as [string];
    expect(url).toBe('https://api.no-tickets.com/v1/tokens/tok%2Fwith%2Fslashes');
  });

  it('exits 1 and mentions the missing argument when token id is absent', async () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_session_secret');

    await runCli(['token', 'revoke']);

    expect(process.exitCode).toBe(1);
    expect(fetchSpy).not.toHaveBeenCalled();
    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining('<tokenId>'));
  });

  it('exits 1 and reports the API error when the server returns a failure', async () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_session_secret');
    fetchSpy.mockReturnValue(jsonResponse({ error: 'token not found' }, 404));

    await runCli(['token', 'revoke', 'tok-missing']);

    expect(process.exitCode).toBe(1);
    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining('token not found'));
  });

  it('exits 1 when not authenticated', async () => {
    await runCli(['token', 'revoke', 'tok-1']);

    expect(process.exitCode).toBe(1);
    expect(fetchSpy).not.toHaveBeenCalled();
    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining('Not authenticated'));
  });
});

describe('token unknown subcommand', () => {
  it('exits 1 and prints a hint listing valid subcommands', async () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_session_secret');

    await runCli(['token', 'rotate']);

    expect(process.exitCode).toBe(1);
    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining('list | create | revoke'));
  });

  it('surfaces "(none)" when no subcommand is given', async () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_session_secret');

    await runCli(['token']);

    expect(process.exitCode).toBe(1);
    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining('(none)'));
  });
});
