import type { Client } from '../transport/client.js';
import { listEventTypes } from './client.js';
import { readCache, writeCache, type CacheFile } from './cache.js';

export type RefreshResult =
  | { readonly status: '200'; readonly etag: string }
  | { readonly status: '304'; readonly etag: string }
  | { readonly status: 'failed'; readonly error: unknown };

export type AwaitRefreshResult = RefreshResult | { readonly status: 'timeout' };

const inflight = new Map<string, Promise<RefreshResult>>();

/** Test-only escape hatch: clear the per-server inflight map between cases. */
export function __resetRefreshState(): void {
  inflight.clear();
}

async function performRefresh(client: Client): Promise<RefreshResult> {
  const baseUrl = client.baseUrl;
  try {
    const existing = readCache(baseUrl);
    const result = await listEventTypes(client, {
      ifNoneMatch: existing?.etag,
    });

    const fetchedAt = new Date().toISOString();

    if ('status' in result) {
      // 304: leave types/etag, update fetchedAt only (when we have a cache).
      if (existing !== null) {
        writeCache(baseUrl, { ...existing, fetchedAt });
      }
      return { status: '304', etag: result.etag };
    }

    const file: CacheFile = {
      version: 1,
      etag: result.etag,
      fetchedAt,
      serverUrl: baseUrl,
      types: [...result.types],
    };
    writeCache(baseUrl, file);
    return { status: '200', etag: result.etag };
  } catch (error) {
    return { status: 'failed', error };
  }
}

/** Schedule an async refresh for the registry cache keyed on client.baseUrl.
 *  At most one inflight refresh per server URL within a process; concurrent
 *  callers receive the same promise (coalesced). Failures resolve with
 *  { status: 'failed' } rather than throwing — PRD requires refresh failures
 *  to log at debug level and not break user-facing commands. */
export function scheduleRefresh(client: Client): Promise<RefreshResult> {
  const key = client.baseUrl;
  const existing = inflight.get(key);
  if (existing !== undefined) return existing;

  const promise = performRefresh(client).finally(() => {
    inflight.delete(key);
  });
  inflight.set(key, promise);
  return promise;
}

/** Race the refresh promise against a bounded timeout. Returns the refresh
 *  result if it resolves in time; otherwise { status: 'timeout' }. The
 *  underlying refresh continues to run regardless. */
export function awaitRefresh(
  refresh: Promise<RefreshResult>,
  options: { readonly timeoutMs: number },
): Promise<AwaitRefreshResult> {
  return new Promise<AwaitRefreshResult>((resolve) => {
    const timer = setTimeout(() => {
      resolve({ status: 'timeout' });
    }, options.timeoutMs);
    refresh.then(
      (result) => {
        clearTimeout(timer);
        resolve(result);
      },
      (error) => {
        clearTimeout(timer);
        resolve({ status: 'failed', error });
      },
    );
  });
}
