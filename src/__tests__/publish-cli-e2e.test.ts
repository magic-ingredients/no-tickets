import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runCli } from '../cli.js';

// Phase-1 contract for `nt publish <type-id> <data-json>`:
// - Dispatcher in cli.ts routes 'publish' to runPublishSingle
// - validateEventLocally (consuming bundled @nt/schemas) gates the publish
// - On valid data, POSTs to <NO_TICKETS_API_URL>/v1/events with Bearer auth
// - On unknown type, exits 2 without an HTTP call
// - On schema-invalid data, exits 1 without an HTTP call
//
// The --project / --token / --token-stdin / --token-env-var precedence
// described in the fix doc lands later (Tasks 2+3 give us the registry).
// This e2e covers the env-var auth path that ships in this slice.

let testDir: string;
let logSpy: ReturnType<typeof vi.spyOn>;
let errSpy: ReturnType<typeof vi.spyOn>;
let fetchSpy: ReturnType<typeof vi.spyOn>;

const PUBLISH_OK_BODY = JSON.stringify({ ingested: 1, deduped: 0, ids: ['evt-1'] });

function happyFetchResponse(): Response {
  return new Response(PUBLISH_OK_BODY, {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}

beforeEach(async () => {
  testDir = await mkdtemp(join(tmpdir(), 'nt-publish-e2e-'));
  vi.stubEnv('NO_TICKETS_HOME', testDir);
  vi.stubEnv('NO_TICKETS_TOKEN', 'nt_push_e2etest');
  vi.stubEnv('NO_TICKETS_API_URL', 'https://api.test.example');
  vi.stubEnv('NO_TICKETS_AUTH_URL', 'https://app.test.example/api/auth/cli');
  process.exitCode = undefined;
  logSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
  errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(happyFetchResponse());
});

afterEach(async () => {
  vi.unstubAllEnvs();
  vi.restoreAllMocks();
  await rm(testDir, { recursive: true, force: true });
});

describe('publish command e2e — env-var auth', () => {
  it('publishes a valid event to <api-url>/v1/events with Bearer auth', async () => {
    await runCli([
      'publish',
      'product.epic.created.v1',
      '{"epicId":"e1","projectId":"p1","title":"my epic"}',
    ]);

    expect(process.exitCode).toBeFalsy();
    expect(fetchSpy).toHaveBeenCalledOnce();
    const [url, init] = fetchSpy.mock.calls[0] as [string | URL, RequestInit | undefined];
    expect(String(url)).toBe('https://api.test.example/v1/events');
    expect(init?.method).toBe('POST');

    // Authorization header carries the env token
    const headers = new Headers(init?.headers as HeadersInit);
    expect(headers.get('authorization')).toBe('Bearer nt_push_e2etest');
    expect(headers.get('content-type')).toMatch(/application\/json/);

    const body = JSON.parse(init?.body as string) as Array<{
      type: string;
      data: Record<string, unknown>;
    }>;
    expect(Array.isArray(body)).toBe(true);
    expect(body[0]?.type).toBe('product.epic.created.v1');
    expect(body[0]?.data).toEqual({ epicId: 'e1', projectId: 'p1', title: 'my epic' });
  });

  it('exits 2 (unknown_event_type) without an HTTP call when type id is not in the bundled registry', async () => {
    await runCli(['publish', 'definitely.not.a.thing.v9', '{}']);

    expect(process.exitCode).toBe(2);
    expect(fetchSpy).not.toHaveBeenCalled();

    const errOutput = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(errOutput).toMatch(/unknown.*type/i);
  });

  it('exits 1 (validation) without an HTTP call when data fails the bundled schema', async () => {
    // product.epic.created.v1 requires epicId, projectId, title — empty object fails all
    await runCli(['publish', 'product.epic.created.v1', '{}']);

    expect(process.exitCode).toBe(1);
    expect(fetchSpy).not.toHaveBeenCalled();

    const errOutput = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    // At least one of the missing required fields should be mentioned in the error stream
    expect(errOutput).toMatch(/epicId|projectId|title/);
  });

  it('exits non-zero with an auth error when NO_TICKETS_TOKEN is not set', async () => {
    vi.stubEnv('NO_TICKETS_TOKEN', '');
    await runCli([
      'publish',
      'product.epic.created.v1',
      '{"epicId":"e1","projectId":"p1","title":"t"}',
    ]);

    expect(process.exitCode).toBeGreaterThan(0);
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it('exits 1 with a usage error when type-id positional is missing', async () => {
    await runCli(['publish']);

    expect(process.exitCode).toBe(1);
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it('prints the published id on success', async () => {
    await runCli([
      'publish',
      'product.epic.created.v1',
      '{"epicId":"e1","projectId":"p1","title":"t"}',
    ]);

    const out = logSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(out).toMatch(/evt-1/);
  });
});
