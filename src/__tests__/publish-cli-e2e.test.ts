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
  vi.spyOn(console, 'log').mockImplementation(() => {});
  errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(happyFetchResponse());
});

afterEach(async () => {
  vi.unstubAllEnvs();
  vi.restoreAllMocks();
  await rm(testDir, { recursive: true, force: true });
});

describe('publish command e2e — --project resolution via ~/.notickets/config.json', () => {
  it('resolves token + apiUrl from the registered project', async () => {
    const { mkdir, writeFile } = await import('node:fs/promises');
    await mkdir(join(testDir, '.notickets'), { recursive: true });
    await writeFile(
      join(testDir, '.notickets', 'config.json'),
      JSON.stringify({
        profiles: {
          mystaging: {
            apiUrl: 'https://api-from-config.example.com',
            authUrl: 'https://app-from-config.example.com/api/auth/cli',
          },
        },
        projects: {
          myapp: { profile: 'mystaging', pushToken: 'nt_push_from_config' },
        },
      }),
    );

    // Clear env-var token so the new mutual-exclusion guard doesn't
    // reject the call. (Conflict behavior is exercised by a separate
    // test below.)
    vi.stubEnv('NO_TICKETS_TOKEN', '');

    await runCli([
      'publish',
      'product.epic.created.v1',
      '{"epicId":"e1","projectId":"p1","title":"t"}',
      '--project',
      'myapp',
    ]);

    expect(process.exitCode).toBeFalsy();
    expect(fetchSpy).toHaveBeenCalledOnce();
    const [url, init] = fetchSpy.mock.calls[0] as [string | URL, RequestInit];
    // URL came from the project's profile (not from NO_TICKETS_API_URL,
    // which was set in beforeEach to a different host)
    expect(String(url)).toBe('https://api-from-config.example.com/v1/events');
    // Token came from the project's pushToken
    const headers = new Headers(init.headers as HeadersInit);
    expect(headers.get('authorization')).toBe('Bearer nt_push_from_config');
  });

  it('exits 1 with ProjectNotRegisteredError message when --project name is unknown', async () => {
    const { mkdir, writeFile } = await import('node:fs/promises');
    await mkdir(join(testDir, '.notickets'), { recursive: true });
    await writeFile(
      join(testDir, '.notickets', 'config.json'),
      JSON.stringify({ profiles: {}, projects: {} }),
    );
    vi.stubEnv('NO_TICKETS_TOKEN', '');

    await runCli([
      'publish',
      'product.epic.created.v1',
      '{"epicId":"e","projectId":"p","title":"t"}',
      '--project',
      'no-such-project',
    ]);

    expect(process.exitCode).toBe(1);
    expect(fetchSpy).not.toHaveBeenCalled();
    const err = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(err).toMatch(/no-such-project/);
    expect(err).toMatch(/not registered/);
  });

  it('exits 1 with the file-path guidance when --project is supplied but config.json does not exist', async () => {
    // Don't write config.json. NO_TICKETS_HOME points at an empty tmpdir.
    vi.stubEnv('NO_TICKETS_TOKEN', '');
    await runCli([
      'publish',
      'product.epic.created.v1',
      '{"epicId":"e","projectId":"p","title":"t"}',
      '--project',
      'mystaging',
    ]);

    expect(process.exitCode).toBe(1);
    expect(fetchSpy).not.toHaveBeenCalled();
    const err = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(err).toMatch(/mystaging/);
    // AND, not OR — the message must name the file so the user knows
    // where to fix. Earlier `/config\.json|not registered/i` would have
    // passed even if the path guidance was silently dropped.
    expect(err).toMatch(/config\.json/);
    // Pin the actionable hint that points at `nt project link`
    expect(err).toMatch(/nt project link/);
  });

  it('rejects --project + NO_TICKETS_TOKEN as mutually exclusive (no silent precedence)', async () => {
    // The previous design silently preferred --project. A user who set
    // NO_TICKETS_TOKEN for one project and then ran with --project
    // pointing at another would silently publish to the second — wrong
    // semantics. Reject the combination up front.
    const { mkdir, writeFile } = await import('node:fs/promises');
    await mkdir(join(testDir, '.notickets'), { recursive: true });
    await writeFile(
      join(testDir, '.notickets', 'config.json'),
      JSON.stringify({
        profiles: { staging: { apiUrl: 'https://x', authUrl: 'https://x/auth' } },
        projects: { myapp: { profile: 'staging', pushToken: 'nt_push_x' } },
      }),
    );

    // env-var-token already set in beforeEach
    await runCli([
      'publish',
      'product.epic.created.v1',
      '{"epicId":"e","projectId":"p","title":"t"}',
      '--project',
      'myapp',
    ]);

    expect(process.exitCode).toBe(1);
    expect(fetchSpy).not.toHaveBeenCalled();
    const err = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(err).toMatch(/--project.*NO_TICKETS_TOKEN.*mutually exclusive/);
  });

  it('rejects --project + --profile as mutually exclusive (--project carries its own profile reference)', async () => {
    const { mkdir, writeFile } = await import('node:fs/promises');
    await mkdir(join(testDir, '.notickets'), { recursive: true });
    await writeFile(
      join(testDir, '.notickets', 'config.json'),
      JSON.stringify({
        profiles: { staging: { apiUrl: 'https://x', authUrl: 'https://x/auth' } },
        projects: { myapp: { profile: 'staging', pushToken: 'nt_push_x' } },
      }),
    );
    vi.stubEnv('NO_TICKETS_TOKEN', '');

    await runCli([
      'publish',
      'product.epic.created.v1',
      '{"epicId":"e","projectId":"p","title":"t"}',
      '--project',
      'myapp',
      '--profile',
      'staging',
    ]);

    expect(process.exitCode).toBe(1);
    expect(fetchSpy).not.toHaveBeenCalled();
    const err = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(err).toMatch(/--project.*--profile.*mutually exclusive/);
  });

  it('rejects bare --project (no value) instead of silently using env-var auth', async () => {
    // Without this guard, `--project` with no following value parses as
    // `flags.project = true`, flagString returns undefined, and the
    // env-var auth path takes over silently. A user who typo'd the
    // project name (e.g. `--project<TAB>` and forgot to fill in) would
    // publish to whichever project NO_TICKETS_TOKEN belongs to.
    await runCli([
      'publish',
      'product.epic.created.v1',
      '{"epicId":"e","projectId":"p","title":"t"}',
      '--project',
    ]);

    expect(process.exitCode).toBe(1);
    expect(fetchSpy).not.toHaveBeenCalled();
    const err = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(err).toMatch(/--project requires a value/);
  });
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

  it('exits 1 with the canonical "Not authenticated" message when NO_TICKETS_TOKEN is unset and no credentials file exists', async () => {
    // Pin both: that auth resolution was attempted (canonical message), and
    // that no fetch happened. Without the message check, the test would pass
    // even if a stored credentials file silently picked up the publish.
    vi.stubEnv('NO_TICKETS_TOKEN', '');
    await runCli([
      'publish',
      'product.epic.created.v1',
      '{"epicId":"e1","projectId":"p1","title":"t"}',
    ]);

    expect(process.exitCode).toBe(1);
    expect(fetchSpy).not.toHaveBeenCalled();
    const errOutput = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(errOutput).toMatch(/Not authenticated/);
  });

  it('exits 1 with the "<type-id> is required" usage message when the positional is missing', async () => {
    await runCli(['publish']);

    expect(process.exitCode).toBe(1);
    expect(fetchSpy).not.toHaveBeenCalled();
    // Pin which guard fires — multiple branches in handlePublish can return
    // exit 1, so the message is the contract.
    const errOutput = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(errOutput).toMatch(/<type-id> is required/);
  });

  it('reads --data from stdin when "-" is passed as the data positional', async () => {
    // The handler wires runPublishSingle's readStdin to process.stdin; verify
    // the integration end-to-end. Without this test the readStdinAll helper
    // and the "-" sentinel path are unexercised at the CLI seam.
    const stdinPayload = JSON.stringify({
      epicId: 'from-stdin',
      projectId: 'p1',
      title: 'piped',
    });
    const stdinSpy = vi
      .spyOn(process.stdin, Symbol.asyncIterator as unknown as 'on')
      .mockImplementation(
        () =>
          (async function* () {
            yield Buffer.from(stdinPayload);
          })() as unknown as ReturnType<typeof process.stdin.on>,
      );

    try {
      await runCli(['publish', 'product.epic.created.v1', '-']);

      expect(process.exitCode).toBeFalsy();
      expect(fetchSpy).toHaveBeenCalledOnce();
      const [, init] = fetchSpy.mock.calls[0] as [string | URL, RequestInit | undefined];
      const body = JSON.parse(init?.body as string) as Array<{ data: { epicId: string } }>;
      expect(body[0]?.data.epicId).toBe('from-stdin');
    } finally {
      stdinSpy.mockRestore();
    }
  });
});
