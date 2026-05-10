import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { mkdtempSync, rmSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { Client } from '../transport/client.js';
import { createEvents, CACHE_MISSING_MESSAGE } from './index.js';
import { writeCache, cachePath, readCache, type CacheFile } from './cache.js';
import type { EventTypeSpec } from './client.js';

const TYPE_A: EventTypeSpec = {
  id: 'app.user.signed-up.v1',
  domain: 'app.user',
  entity: 'user',
  action: 'signed-up',
  version: 'v1',
  schema: { type: 'object', properties: {} },
  deprecatedAt: null,
};

const TYPE_B: EventTypeSpec = {
  id: 'engineering.deploy.completed.v1',
  domain: 'engineering',
  entity: 'deploy',
  action: 'completed',
  version: 'v1',
  schema: { type: 'object', properties: {} },
  deprecatedAt: '2026-01-01T00:00:00Z',
};

// TYPE_C has no `deprecatedAt` field at all (vs TYPE_A's explicit null).
// Pins isDeprecated's "undefined" branch separately from the "null" branch.
const TYPE_C: EventTypeSpec = {
  id: 'app.session.started.v1',
  domain: 'app.session',
  entity: 'session',
  action: 'started',
  version: 'v1',
  schema: { type: 'object', properties: {} },
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
  // Use NO_TICKETS_HOME directly: cache.ts checks it first, bypassing
  // os.homedir(). This keeps test isolation robust in Stryker's subprocess
  // workers where vi.stubEnv('HOME') doesn't propagate into homedir().
  process.env['NO_TICKETS_HOME'] = tempHome;
  cwdSpy = vi.spyOn(process, 'cwd').mockReturnValue(tempCwd);
});

afterEach(() => {
  cwdSpy.mockRestore();
  delete process.env['NO_TICKETS_HOME'];
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

  it('throws a clear, actionable diagnostic when the cache is missing', async () => {
    const events = createEvents({ client: makeClient() });

    await expect(events.list()).rejects.toThrow(CACHE_MISSING_MESSAGE);
  });

  it('treats deprecatedAt: null as NOT deprecated (matches PRD example shape)', async () => {
    // TYPE_A has deprecatedAt: null explicitly. With deprecated: false it
    // must surface; with deprecated: true it must NOT.
    writeCache(BASE_URL, buildCache());
    const events = createEvents({ client: makeClient() });

    expect(await events.list({ deprecated: false })).toEqual([TYPE_A]);
    expect(await events.list({ deprecated: true })).toEqual([TYPE_B]);
  });

  it('treats deprecatedAt as not present (omitted) the same as null — both NOT deprecated', async () => {
    // TYPE_C has no deprecatedAt key at all.
    writeCache(BASE_URL, { ...buildCache(), types: [TYPE_C] });
    const events = createEvents({ client: makeClient() });

    expect(await events.list({ deprecated: false })).toEqual([TYPE_C]);
    expect(await events.list({ deprecated: true })).toEqual([]);
  });

  it('returns an empty array when the cached types list is empty', async () => {
    writeCache(BASE_URL, { ...buildCache(), types: [] });
    const events = createEvents({ client: makeClient() });

    expect(await events.list()).toEqual([]);
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
    // Pin the trigger too, so a regression that drops the call is caught.
    expect(scheduleRefresh).toHaveBeenCalledTimes(1);

    releaseRefresh();
  });

  it('swallows scheduleRefresh rejections so the read returns cleanly (offline-with-cache)', async () => {
    writeCache(BASE_URL, buildCache());
    const scheduleRefresh = vi.fn().mockRejectedValue(new Error('offline'));
    const events = createEvents({ client: makeClient(), scheduleRefresh });

    const types = await events.list();

    expect(types).toEqual([TYPE_A, TYPE_B]);
  });

  it('swallows synchronous throws from scheduleRefresh', async () => {
    writeCache(BASE_URL, buildCache());
    const scheduleRefresh = vi.fn(() => {
      throw new Error('sync boom');
    });
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
    const events = createEvents({ client: makeClient(jsonFetch({ eventType: TYPE_B })) });

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
    const events = createEvents({ client: makeClient(jsonFetch({ eventType: TYPE_A })) });

    const result = await events.describe(TYPE_A.id);

    expect(result).toEqual(TYPE_A);
    // No cache file should have been written without a known ETag.
    expect(() => readFileSync(cachePath(BASE_URL), 'utf-8')).toThrow();
    expect(readCache(BASE_URL)).toBeNull();
  });

  it('triggers scheduleRefresh on a full cache miss (no cache file exists)', async () => {
    const scheduleRefresh = vi.fn().mockResolvedValue(undefined);
    const events = createEvents({
      client: makeClient(jsonFetch({ eventType: TYPE_A })),
      scheduleRefresh,
    });

    await events.describe(TYPE_A.id);

    expect(scheduleRefresh).toHaveBeenCalledTimes(1);
  });

  it('triggers scheduleRefresh on cache hit', async () => {
    writeCache(BASE_URL, buildCache());
    const scheduleRefresh = vi.fn().mockResolvedValue(undefined);
    const events = createEvents({ client: makeClient(), scheduleRefresh });

    await events.describe(TYPE_A.id);

    expect(scheduleRefresh).toHaveBeenCalledTimes(1);
  });

  it('triggers scheduleRefresh on cache miss (after the network call)', async () => {
    const seed = { ...buildCache(), types: [TYPE_A] };
    writeCache(BASE_URL, seed);
    const scheduleRefresh = vi.fn().mockResolvedValue(undefined);
    const events = createEvents({
      client: makeClient(jsonFetch({ eventType: TYPE_B })),
      scheduleRefresh,
    });

    await events.describe(TYPE_B.id);

    expect(scheduleRefresh).toHaveBeenCalledTimes(1);
  });

  it('skips the writeback when a concurrent refresh already added the type', async () => {
    // Race scenario: between describe's read and its post-fetch write, the
    // refresh worker overwrites the cache with a fuller list that already
    // contains TYPE_B. Describe must NOT clobber that with its own merge.
    const seed: CacheFile = { ...buildCache(), types: [TYPE_A] };
    writeCache(BASE_URL, seed);

    let networkCall = 0;
    const fetchImpl: typeof fetch = async () => {
      networkCall++;
      // Simulate the refresh worker landing during the in-flight describe.
      const refreshed: CacheFile = {
        ...seed,
        etag: 'W/"refreshed"',
        types: [TYPE_A, TYPE_B],
      };
      writeCache(BASE_URL, refreshed);
      // Detail endpoint wraps the spec under `eventType`.
      return new Response(JSON.stringify({ eventType: TYPE_B }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      });
    };

    const events = createEvents({ client: makeClient(fetchImpl) });
    const result = await events.describe(TYPE_B.id);

    expect(networkCall).toBe(1);
    expect(result).toEqual(TYPE_B);
    // Cache must still carry the refresh worker's ETag; describe must NOT
    // have rewritten with the stale (pre-fetch) snapshot.
    const after = readCache(BASE_URL);
    expect(after?.etag).toBe('W/"refreshed"');
    expect(after?.types).toEqual([TYPE_A, TYPE_B]);
  });
});

describe('events — base path resolution', () => {
  it('uses Client.baseUrl as the cache key (multi-server isolation)', async () => {
    // Distinguishable contents so a broken cache key (e.g. shared file)
    // would surface as "wrong server's data" rather than just "any data".
    const aOnly: EventTypeSpec = { ...TYPE_A, id: 'server-a.only.v1' };
    const bOnly: EventTypeSpec = { ...TYPE_B, id: 'server-b.only.v1' };
    writeCache(BASE_URL, { ...buildCache(), serverUrl: BASE_URL, types: [aOnly] });
    writeCache('https://other.example.com', {
      ...buildCache(),
      serverUrl: 'https://other.example.com',
      types: [bOnly],
    });

    const aClient = new Client({ baseUrl: BASE_URL, token: 't', fetch: vi.fn(), source: { name: 'sdk', sdkVersion: 'x' } });
    const bClient = new Client({ baseUrl: 'https://other.example.com', token: 't', fetch: vi.fn(), source: { name: 'sdk', sdkVersion: 'x' } });

    const a = await createEvents({ client: aClient }).list();
    const b = await createEvents({ client: bClient }).list();

    expect(a.map((t) => t.id)).toEqual(['server-a.only.v1']);
    expect(b.map((t) => t.id)).toEqual(['server-b.only.v1']);
  });
});

