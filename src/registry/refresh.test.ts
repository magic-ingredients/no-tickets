import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { mkdtempSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { Client } from '../transport/client.js';
import { scheduleRefresh, awaitRefresh, __resetRefreshState } from './refresh.js';
import { writeCache, readCache, type CacheFile } from './cache.js';
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
};

const BASE_URL = 'https://api.example.com';

let tempHome: string;
let tempCwd: string;
let cwdSpy: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  tempHome = mkdtempSync(join(tmpdir(), 'no-tickets-refresh-home-'));
  tempCwd = mkdtempSync(join(tmpdir(), 'no-tickets-refresh-cwd-'));
  process.env['NO_TICKETS_HOME'] = tempHome;
  cwdSpy = vi.spyOn(process, 'cwd').mockReturnValue(tempCwd);
  __resetRefreshState();
});

afterEach(() => {
  cwdSpy.mockRestore();
  delete process.env['NO_TICKETS_HOME'];
  rmSync(tempHome, { recursive: true, force: true });
  rmSync(tempCwd, { recursive: true, force: true });
});

interface MockResponseInit {
  readonly status?: number;
  readonly body?: unknown;
  readonly headers?: Record<string, string>;
}

function jsonResponse(init: MockResponseInit = {}): Response {
  const status = init.status ?? 200;
  const bodyText = init.body === undefined ? '' : JSON.stringify(init.body);
  const headers: Record<string, string> = { 'content-type': 'application/json', ...init.headers };
  return new Response(bodyText, { status, headers });
}

function makeClient(fetchImpl?: typeof fetch): Client {
  return new Client({
    baseUrl: BASE_URL,
    token: 't',
    fetch: fetchImpl ?? vi.fn(),
    source: { name: 'sdk', sdkVersion: '9.9.9-test' },
  });
}

function seedCache(overrides: Partial<CacheFile> = {}): CacheFile {
  const file: CacheFile = {
    version: 1,
    etag: 'W/"seed"',
    fetchedAt: '2026-04-27T10:23:00Z',
    serverUrl: BASE_URL,
    types: [TYPE_A],
    ...overrides,
  };
  writeCache(BASE_URL, file);
  return file;
}

describe('scheduleRefresh — fresh fetch (no prior cache)', () => {
  it('writes a new CacheFile with the server etag and types', async () => {
    const headers = { etag: 'W/"new"' };
    const fetchImpl: typeof fetch = async () =>
      jsonResponse({ body: { types: [TYPE_A, TYPE_B] }, headers });

    const result = await scheduleRefresh(makeClient(fetchImpl));

    expect(result.status).toBe('200');
    const cache = readCache(BASE_URL);
    expect(cache?.etag).toBe('W/"new"');
    expect(cache?.types).toEqual([TYPE_A, TYPE_B]);
  });

  it('does NOT send If-None-Match when there is no prior cache', async () => {
    let captured: Headers | undefined;
    const fetchImpl: typeof fetch = async (_url, init) => {
      captured = new Headers(init?.headers);
      return jsonResponse({ body: { types: [] }, headers: { etag: 'W/"x"' } });
    };

    await scheduleRefresh(makeClient(fetchImpl));

    expect(captured?.get('if-none-match')).toBeNull();
  });
});

describe('scheduleRefresh — conditional refresh', () => {
  it('sends If-None-Match using the cached etag when a prior cache exists', async () => {
    seedCache();
    let captured: Headers | undefined;
    const fetchImpl: typeof fetch = async (_url, init) => {
      captured = new Headers(init?.headers);
      return jsonResponse({ body: { types: [TYPE_A] }, headers: { etag: 'W/"new"' } });
    };

    await scheduleRefresh(makeClient(fetchImpl));

    expect(captured?.get('if-none-match')).toBe('W/"seed"');
  });

  it('on 304: leaves the cache types/etag untouched but updates fetchedAt', async () => {
    const seeded = seedCache({ fetchedAt: '2020-01-01T00:00:00Z' });
    const fetchImpl: typeof fetch = async () =>
      new Response(null, { status: 304, headers: { etag: 'W/"seed"' } });

    const result = await scheduleRefresh(makeClient(fetchImpl));

    expect(result.status).toBe('304');
    const after = readCache(BASE_URL);
    expect(after?.etag).toBe(seeded.etag);
    expect(after?.types).toEqual(seeded.types);
    expect(after?.fetchedAt).not.toBe(seeded.fetchedAt);
  });

  it('on 200: replaces the cache atomically with the new types and etag', async () => {
    seedCache();
    const fetchImpl: typeof fetch = async () =>
      jsonResponse({
        body: { types: [TYPE_A, TYPE_B] },
        headers: { etag: 'W/"updated"' },
      });

    const result = await scheduleRefresh(makeClient(fetchImpl));

    expect(result.status).toBe('200');
    const after = readCache(BASE_URL);
    expect(after?.etag).toBe('W/"updated"');
    expect(after?.types).toEqual([TYPE_A, TYPE_B]);
  });
});

describe('scheduleRefresh — failure handling', () => {
  it('does not throw when the network call fails (PRD: log at debug, do not error)', async () => {
    seedCache();
    const fetchImpl: typeof fetch = async () => {
      throw new Error('offline');
    };

    const result = await scheduleRefresh(makeClient(fetchImpl));

    expect(result.status).toBe('failed');
  });

  it('leaves the prior cache intact on failure', async () => {
    const seeded = seedCache();
    const fetchImpl: typeof fetch = async () => {
      throw new Error('offline');
    };

    await scheduleRefresh(makeClient(fetchImpl));

    expect(readCache(BASE_URL)).toEqual(seeded);
  });

  it('does not throw on 5xx server errors', async () => {
    seedCache();
    const fetchImpl: typeof fetch = async () =>
      jsonResponse({ status: 503, body: { msg: 'unavail' } });

    const result = await scheduleRefresh(makeClient(fetchImpl));

    expect(result.status).toBe('failed');
  });
});

describe('scheduleRefresh — bounded concurrency', () => {
  it('coalesces parallel in-process calls for the same server (one inflight)', async () => {
    let callCount = 0;
    const fetchImpl: typeof fetch = async () => {
      callCount++;
      // Yield so multiple concurrent calls can stack up before we resolve.
      await new Promise((resolve) => setTimeout(resolve, 20));
      return jsonResponse({ body: { types: [] }, headers: { etag: 'W/"x"' } });
    };
    const client = makeClient(fetchImpl);

    const [a, b, c] = await Promise.all([
      scheduleRefresh(client),
      scheduleRefresh(client),
      scheduleRefresh(client),
    ]);

    expect(callCount).toBe(1);
    // All three callers see the same result.
    expect(a).toEqual(b);
    expect(b).toEqual(c);
  });

  it('does NOT coalesce calls across different server URLs', async () => {
    let callCount = 0;
    const fetchImpl: typeof fetch = async () => {
      callCount++;
      return jsonResponse({ body: { types: [] }, headers: { etag: 'W/"x"' } });
    };
    const a = makeClient(fetchImpl);
    const b = new Client({
      baseUrl: 'https://other.example.com',
      token: 't',
      fetch: fetchImpl,
      source: { name: 'sdk', sdkVersion: 'x' },
    });

    await Promise.all([scheduleRefresh(a), scheduleRefresh(b)]);

    expect(callCount).toBe(2);
  });

  it('starts a fresh refresh after the previous one finished', async () => {
    let callCount = 0;
    const fetchImpl: typeof fetch = async () => {
      callCount++;
      return jsonResponse({ body: { types: [] }, headers: { etag: 'W/"x"' } });
    };
    const client = makeClient(fetchImpl);

    await scheduleRefresh(client);
    await scheduleRefresh(client);

    expect(callCount).toBe(2);
  });
});

describe('awaitRefresh', () => {
  it('returns the refresh result when it resolves before the timeout', async () => {
    seedCache();
    const fetchImpl: typeof fetch = async () =>
      jsonResponse({ body: { types: [] }, headers: { etag: 'W/"x"' } });

    const promise = scheduleRefresh(makeClient(fetchImpl));
    const result = await awaitRefresh(promise, { timeoutMs: 1000 });

    expect(result.status).toBe('200');
  });

  it('returns { status: "timeout" } when the refresh is still inflight', async () => {
    let release!: () => void;
    const blocked = new Promise<void>((r) => {
      release = r;
    });
    const fetchImpl: typeof fetch = async () => {
      await blocked;
      return jsonResponse({ body: { types: [] }, headers: { etag: 'W/"x"' } });
    };

    const promise = scheduleRefresh(makeClient(fetchImpl));
    const result = await awaitRefresh(promise, { timeoutMs: 50 });

    expect(result.status).toBe('timeout');
    release();
    // Ensure the underlying refresh still completes and doesn't unhandled-reject.
    await promise;
  });
});
