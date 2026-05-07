import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { mkdtempSync, rmSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { Client } from '../transport/client.js';
import { createEvents } from './index.js';
import { writeCache, cachePath, type CacheFile } from './cache.js';
import type { EventTypeSpec } from './client.js';

const TYPE_A: EventTypeSpec = {
  id: 'app.user.signed-up.v1',
  domain: 'app.user',
  entity: 'user',
  action: 'signed-up',
  version: 1,
  schema: { type: 'object', properties: {} },
};

const TYPE_B: EventTypeSpec = {
  id: 'engineering.deploy.completed.v1',
  domain: 'engineering',
  entity: 'deploy',
  action: 'completed',
  version: 1,
  schema: { type: 'object', properties: {} },
  deprecatedAt: '2026-01-01T00:00:00Z',
};

const BASE_URL = 'https://api.example.com';

function buildCache(): CacheFile {
  return {
    version: 1,
    etag: 'W/"seed"',
    fetchedAt: '2026-04-27T10:23:00Z',
    serverUrl: BASE_URL,
    types: [TYPE_A, TYPE_B],
  };
}

let tempHome: string;
let tempCwd: string;
let cwdSpy: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  tempHome = mkdtempSync(join(tmpdir(), 'no-tickets-events-home-'));
  tempCwd = mkdtempSync(join(tmpdir(), 'no-tickets-events-cwd-'));
  vi.stubEnv('HOME', tempHome);
  vi.stubEnv('USERPROFILE', tempHome);
  delete process.env['NO_TICKETS_HOME'];
  cwdSpy = vi.spyOn(process, 'cwd').mockReturnValue(tempCwd);
  // No project-local .notickets/ — falls back to tempHome.
});

afterEach(() => {
  cwdSpy.mockRestore();
  vi.unstubAllEnvs();
  rmSync(tempHome, { recursive: true, force: true });
  rmSync(tempCwd, { recursive: true, force: true });
});

function makeClient(fetchImpl?: typeof fetch): Client {
  return new Client({
    baseUrl: BASE_URL,
    token: 't',
    fetch: fetchImpl ?? vi.fn(),
    source: { name: 'sdk', sdkVersion: '9.9.9-test' },
  });
}

function jsonFetch(body: unknown, status = 200): typeof fetch {
  return (async () =>
    new Response(JSON.stringify(body), {
      status,
      headers: { 'content-type': 'application/json' },
    })) as typeof fetch;
}

describe('events.list — cache reads', () => {
  it('returns the cached types when the cache is valid', async () => {
    writeCache(BASE_URL, buildCache());
    const client = makeClient();

    const events = createEvents({ client });
    const types = await events.list();

    expect(types).toEqual([TYPE_A, TYPE_B]);
  });

  it('filters by domain', async () => {
    writeCache(BASE_URL, buildCache());
    const events = createEvents({ client: makeClient() });

    const types = await events.list({ domain: 'engineering' });

    expect(types).toEqual([TYPE_B]);
  });

  it('filters out deprecated types when deprecated: false', async () => {
    writeCache(BASE_URL, buildCache());
    const events = createEvents({ client: makeClient() });

    const types = await events.list({ deprecated: false });

    expect(types).toEqual([TYPE_A]);
  });

  it('returns ONLY deprecated types when deprecated: true', async () => {
    writeCache(BASE_URL, buildCache());
    const events = createEvents({ client: makeClient() });

    const types = await events.list({ deprecated: true });

    expect(types).toEqual([TYPE_B]);
  });

  it('throws a clear diagnostic when the cache is missing', async () => {
    const events = createEvents({ client: makeClient() });

    await expect(events.list()).rejects.toThrow(/cache/i);
  });
});

describe('events.list — refresh trigger', () => {
  it('triggers scheduleRefresh fire-and-forget on every call', async () => {
    writeCache(BASE_URL, buildCache());
    const scheduleRefresh = vi.fn().mockResolvedValue(undefined);
    const events = createEvents({ client: makeClient(), scheduleRefresh });

    await events.list();

    expect(scheduleRefresh).toHaveBeenCalledTimes(1);
    expect(scheduleRefresh.mock.calls[0]?.[0]).toBeInstanceOf(Client);
  });

  it('does NOT await scheduleRefresh — the read returns even if refresh is slow', async () => {
    writeCache(BASE_URL, buildCache());
    let releaseRefresh!: () => void;
    const refreshDone = new Promise<void>((resolve) => {
      releaseRefresh = resolve;
    });
    const scheduleRefresh = vi.fn().mockReturnValue(refreshDone);
    const events = createEvents({ client: makeClient(), scheduleRefresh });

    // If list awaited refresh, this would hang until releaseRefresh().
    const types = await events.list();
    expect(types).toEqual([TYPE_A, TYPE_B]);

    releaseRefresh();
  });

  it('swallows scheduleRefresh rejections so the read returns cleanly (offline-with-cache)', async () => {
    writeCache(BASE_URL, buildCache());
    const scheduleRefresh = vi.fn().mockRejectedValue(new Error('offline'));
    const events = createEvents({ client: makeClient(), scheduleRefresh });

    const types = await events.list();

    expect(types).toEqual([TYPE_A, TYPE_B]);
  });
});

describe('events.describe', () => {
  it('returns the cached type when present', async () => {
    writeCache(BASE_URL, buildCache());
    const events = createEvents({ client: makeClient() });

    const result = await events.describe(TYPE_A.id);

    expect(result).toEqual(TYPE_A);
  });

  it('does NOT hit the network when the cache hits', async () => {
    writeCache(BASE_URL, buildCache());
    const fetchImpl = vi.fn();
    const events = createEvents({ client: makeClient(fetchImpl as typeof fetch) });

    await events.describe(TYPE_A.id);

    expect(fetchImpl).not.toHaveBeenCalled();
  });

  it('falls back to a one-shot fetch on cache miss and writes the new type back to the cache', async () => {
    const seed = { ...buildCache(), types: [TYPE_A] }; // no TYPE_B
    writeCache(BASE_URL, seed);
    const events = createEvents({ client: makeClient(jsonFetch(TYPE_B)) });

    const result = await events.describe(TYPE_B.id);

    expect(result).toEqual(TYPE_B);
    const updated = JSON.parse(readFileSync(cachePath(BASE_URL), 'utf-8')) as CacheFile;
    expect(updated.types.map((t) => t.id)).toContain(TYPE_B.id);
  });

  it('returns null on a 404 from the network (does not write to cache)', async () => {
    const seed = { ...buildCache(), types: [TYPE_A] };
    writeCache(BASE_URL, seed);
    const events = createEvents({
      client: makeClient(jsonFetch({ msg: 'not found' }, 404)),
    });

    const result = await events.describe('app.unknown.v1');

    expect(result).toBeNull();
    const after = JSON.parse(readFileSync(cachePath(BASE_URL), 'utf-8')) as CacheFile;
    expect(after.types).toEqual(seed.types);
  });

  it('falls back to network when the cache is entirely missing (no write)', async () => {
    const events = createEvents({ client: makeClient(jsonFetch(TYPE_A)) });

    const result = await events.describe(TYPE_A.id);

    expect(result).toEqual(TYPE_A);
    // No cache file should have been written without a known ETag.
    expect(() => readFileSync(cachePath(BASE_URL), 'utf-8')).toThrow();
  });
});

describe('events — base path resolution', () => {
  it('uses Client.baseUrl as the cache key (multi-server isolation)', async () => {
    writeCache(BASE_URL, buildCache());
    writeCache('https://other.example.com', { ...buildCache(), serverUrl: 'https://other.example.com' });

    const aClient = new Client({ baseUrl: BASE_URL, token: 't', fetch: vi.fn(), source: { name: 'sdk', sdkVersion: 'x' } });
    const bClient = new Client({ baseUrl: 'https://other.example.com', token: 't', fetch: vi.fn(), source: { name: 'sdk', sdkVersion: 'x' } });

    const a = await createEvents({ client: aClient }).list();
    const b = await createEvents({ client: bClient }).list();

    // Both should succeed — proves cache is keyed per-server.
    expect(a.length).toBeGreaterThan(0);
    expect(b.length).toBeGreaterThan(0);
  });
});

