import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import {
  resolveProjectAuth,
  clientForProject,
  ProjectNotRegisteredError,
  type ResolvedProjectAuth,
} from '../projects.js';
import { Client } from '../../transport/client.js';

// Phase-1 contract for the project registry:
// - resolveProjectAuth(name) reads ~/.notickets/config.json and returns
//   { token, apiUrl, authUrl } by joining projects[name].pushToken with the
//   profile referenced by projects[name].profile.
// - Throws ProjectNotRegisteredError when the project entry is missing.
// - clientForProject(name, overrides?) is the one-line factory: builds and
//   returns a Client with baseUrl + token resolved from the registry.
//   Production code does:  await publish(clientForProject('myapp'), [...])

let testDir: string;

beforeEach(async () => {
  testDir = await mkdtemp(join(tmpdir(), 'nt-projects-'));
  vi.stubEnv('NO_TICKETS_HOME', testDir);
});

afterEach(async () => {
  vi.unstubAllEnvs();
  await rm(testDir, { recursive: true, force: true });
});

async function writeConfig(content: string): Promise<void> {
  await mkdir(join(testDir, '.notickets'), { recursive: true });
  await writeFile(join(testDir, '.notickets', 'config.json'), content);
}

const VALID_CONFIG = JSON.stringify({
  profiles: {
    staging: {
      apiUrl: 'https://api-staging.example.com',
      authUrl: 'https://app-staging.example.com/api/auth/cli',
    },
    production: {
      apiUrl: 'https://api.example.com',
      authUrl: 'https://app.example.com/api/auth/cli',
    },
  },
  projects: {
    myapp: { profile: 'staging', pushToken: 'nt_push_myapp_staging_xxx' },
    'myapp-prd': { profile: 'production', pushToken: 'nt_push_myapp_prod_xxx' },
  },
});

describe('resolveProjectAuth', () => {
  it('returns token + URLs by joining projects[name] with the referenced profile', async () => {
    await writeConfig(VALID_CONFIG);

    const auth: ResolvedProjectAuth = resolveProjectAuth('myapp');
    expect(auth).toEqual({
      token: 'nt_push_myapp_staging_xxx',
      apiUrl: 'https://api-staging.example.com',
      authUrl: 'https://app-staging.example.com/api/auth/cli',
    });
  });

  it('resolves multiple projects from the same config independently', async () => {
    await writeConfig(VALID_CONFIG);

    const staging = resolveProjectAuth('myapp');
    const prod = resolveProjectAuth('myapp-prd');

    expect(staging.token).toBe('nt_push_myapp_staging_xxx');
    expect(staging.apiUrl).toBe('https://api-staging.example.com');
    expect(prod.token).toBe('nt_push_myapp_prod_xxx');
    expect(prod.apiUrl).toBe('https://api.example.com');
  });

  it('throws ProjectNotRegisteredError when the project name is not in config.projects', async () => {
    await writeConfig(VALID_CONFIG);

    expect(() => resolveProjectAuth('unknown-project')).toThrow(ProjectNotRegisteredError);
  });

  it('error message lists available project names so the user can self-diagnose', async () => {
    await writeConfig(VALID_CONFIG);

    try {
      resolveProjectAuth('unknown-project');
      throw new Error('expected throw');
    } catch (err) {
      expect(err).toBeInstanceOf(ProjectNotRegisteredError);
      const message = (err as Error).message;
      expect(message).toMatch(/unknown-project/);
      expect(message).toMatch(/myapp/);
      expect(message).toMatch(/myapp-prd/);
    }
  });

  it('throws ProjectNotRegisteredError when config.json does not exist', async () => {
    // No writeConfig call — file is absent.
    expect(() => resolveProjectAuth('myapp')).toThrow(ProjectNotRegisteredError);
  });

  it('error message points at the config path when file is missing', async () => {
    expect(() => resolveProjectAuth('myapp')).toThrow(/config\.json/);
  });

  it('throws when projects section is missing from a present config', async () => {
    await writeConfig(JSON.stringify({ profiles: { staging: { apiUrl: 'https://x', authUrl: 'https://x' } } }));

    expect(() => resolveProjectAuth('myapp')).toThrow(ProjectNotRegisteredError);
  });

  it('throws when project entry references a profile that is not defined (distinct error from malformed)', async () => {
    await writeConfig(JSON.stringify({
      profiles: {},
      projects: {
        orphan: { profile: 'no-such-profile', pushToken: 'nt_push_x' },
      },
    }));

    // Distinguish "not defined" from "malformed" — both branches mention
    // the profile name, so /no-such-profile/ alone doesn't pin which one.
    expect(() => resolveProjectAuth('orphan')).toThrow(/not defined/);
    expect(() => resolveProjectAuth('orphan')).not.toThrow(/malformed/);
    // Pin the path-naming guidance so a regression dropping it fails.
    expect(() => resolveProjectAuth('orphan')).toThrow(/config\.json/);
  });

  it('throws when project entry is malformed (missing pushToken)', async () => {
    await writeConfig(JSON.stringify({
      profiles: {
        staging: { apiUrl: 'https://x', authUrl: 'https://x/auth' },
      },
      projects: {
        broken: { profile: 'staging' },
      },
    }));

    expect(() => resolveProjectAuth('broken')).toThrow(/pushToken/);
  });

  it('throws when project entry is malformed (missing profile)', async () => {
    await writeConfig(JSON.stringify({
      profiles: {},
      projects: {
        broken: { pushToken: 'nt_push_x' },
      },
    }));

    expect(() => resolveProjectAuth('broken')).toThrow(/profile/);
  });

  it('throws on prototype-name lookups (no prototype-chain leak via Object.hasOwn)', async () => {
    await writeConfig(VALID_CONFIG);

    for (const proto of ['toString', 'hasOwnProperty', 'valueOf']) {
      expect(() => resolveProjectAuth(proto)).toThrow(ProjectNotRegisteredError);
    }
  });

  it('throws on invalid JSON with a path-naming error message', async () => {
    await writeConfig('{this is :not, valid"json"}');

    // Not ProjectNotRegisteredError — the file exists but is corrupt; we
    // surface that as a hard error so the user knows to fix the file
    // rather than treating it as "no projects registered".
    expect(() => resolveProjectAuth('myapp')).toThrow(/invalid JSON/);
    expect(() => resolveProjectAuth('myapp')).toThrow(/config\.json/);
  });

  it('throws when project entry references a profile that exists but is malformed (apiUrl/authUrl missing)', async () => {
    // Distinct from the dangling-profile case: here the profile key IS
    // present in profiles{} but its body is missing required URL fields.
    // The error message should say "malformed", not "not defined".
    await writeConfig(JSON.stringify({
      profiles: {
        broken: { apiUrl: 'https://x' /* authUrl missing */ },
      },
      projects: {
        myapp: { profile: 'broken', pushToken: 'nt_push_x' },
      },
    }));

    expect(() => resolveProjectAuth('myapp')).toThrow(/malformed/);
    expect(() => resolveProjectAuth('myapp')).not.toThrow(/not defined/);
    // Pin that the project + profile names both appear so the user can
    // locate the offending entry without grepping config.json themselves.
    expect(() => resolveProjectAuth('myapp')).toThrow(/myapp/);
    expect(() => resolveProjectAuth('myapp')).toThrow(/broken/);
  });

  it('treats projects: <non-object> in config as "no projects registered"', async () => {
    // Defensive guard: a malformed `projects: 42` value should not be
    // indexed via Object.keys / Object.hasOwn as if it were a record.
    await writeConfig(JSON.stringify({ profiles: {}, projects: 42 }));

    expect(() => resolveProjectAuth('myapp')).toThrow(ProjectNotRegisteredError);
  });

  it('treats projects: null in config as "no projects registered"', async () => {
    // null is typeof 'object' — without the explicit `v !== null` check in
    // isRecord, Object.keys(null) would throw. Pin the guard.
    await writeConfig(JSON.stringify({ profiles: {}, projects: null }));

    expect(() => resolveProjectAuth('myapp')).toThrow(ProjectNotRegisteredError);
  });

  it('treats profiles: null in config as a missing profile (project entry → malformed-or-not-defined)', async () => {
    // Same guard, profiles side. Without `v !== null`, indexing
    // null[entry.profile] would throw a TypeError instead of producing
    // the user-friendly "not defined" / "malformed" message.
    await writeConfig(JSON.stringify({
      profiles: null,
      projects: { myapp: { profile: 'staging', pushToken: 'nt_push_x' } },
    }));

    // Should not throw a low-level TypeError; should surface the
    // not-defined error path with the profile name.
    expect(() => resolveProjectAuth('myapp')).toThrow(/not defined/);
  });
});

describe('clientForProject', () => {
  it('returns a Client wired to the project token and apiUrl', async () => {
    await writeConfig(VALID_CONFIG);

    const client = clientForProject('myapp');
    expect(client).toBeInstanceOf(Client);
  });

  it('Client built from clientForProject sends the project token as Bearer auth', async () => {
    await writeConfig(VALID_CONFIG);

    const fetchSpy = vi
      .fn()
      .mockResolvedValue(new Response('{}', { status: 200, headers: { 'content-type': 'application/json' } }));
    const client = clientForProject('myapp', { fetch: fetchSpy });

    await client.fetchRaw('GET', '/v1/test');

    expect(fetchSpy).toHaveBeenCalledOnce();
    const init = fetchSpy.mock.calls[0]?.[1] as RequestInit;
    const headers = new Headers(init.headers as HeadersInit);
    expect(headers.get('authorization')).toBe('Bearer nt_push_myapp_staging_xxx');
  });

  it('Client built from clientForProject targets the project profile apiUrl', async () => {
    await writeConfig(VALID_CONFIG);

    const fetchSpy = vi
      .fn()
      .mockResolvedValue(new Response('{}', { status: 200, headers: { 'content-type': 'application/json' } }));
    const client = clientForProject('myapp-prd', { fetch: fetchSpy });

    await client.fetchRaw('GET', '/v1/test');

    const url = fetchSpy.mock.calls[0]?.[0];
    expect(String(url)).toBe('https://api.example.com/v1/test');
  });

  it('caller-supplied overrides win over the registry-resolved values', async () => {
    await writeConfig(VALID_CONFIG);

    const fetchSpy = vi
      .fn()
      .mockResolvedValue(new Response('{}', { status: 200, headers: { 'content-type': 'application/json' } }));
    const client = clientForProject('myapp', {
      token: 'nt_push_override_zzz',
      fetch: fetchSpy,
    });

    await client.fetchRaw('GET', '/v1/test');

    const init = fetchSpy.mock.calls[0]?.[1] as RequestInit;
    const headers = new Headers(init.headers as HeadersInit);
    expect(headers.get('authorization')).toBe('Bearer nt_push_override_zzz');
  });

  it('throws ProjectNotRegisteredError when the project name is unknown', async () => {
    await writeConfig(VALID_CONFIG);
    expect(() => clientForProject('unknown')).toThrow(ProjectNotRegisteredError);
  });

  it('exercises the production publish path: publish(client, [event]) carries Bearer auth + correct apiUrl', async () => {
    // Production callers don't use Client#fetchRaw — they go through
    // publish() → Client#request. Pin that the registry-resolved values
    // flow through the entire publish pipeline, not just the raw HTTP seam.
    await writeConfig(VALID_CONFIG);

    const fetchSpy = vi
      .fn()
      .mockResolvedValue(
        new Response(JSON.stringify({ ingested: 1, deduped: 0, ids: ['evt-x'] }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        }),
      );
    const client = clientForProject('myapp', { fetch: fetchSpy });

    const { publish } = await import('../../transport/events.js');
    await publish(client, [
      { type: 'product.epic.created.v1', data: { epicId: 'e', projectId: 'p', title: 't' } },
    ]);

    expect(fetchSpy).toHaveBeenCalledOnce();
    const [url, init] = fetchSpy.mock.calls[0] as [string | URL, RequestInit];
    expect(String(url)).toBe('https://api-staging.example.com/v1/events');
    const headers = new Headers(init.headers as HeadersInit);
    expect(headers.get('authorization')).toBe('Bearer nt_push_myapp_staging_xxx');
  });
});
