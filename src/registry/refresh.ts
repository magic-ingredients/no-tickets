import type { Client, TransportLogger } from '../transport/client.js';
import { listEventTypes } from './client.js';
import { readCache, writeCache, type CacheFile } from './cache.js';

export type RefreshResult =
  | { readonly status: 'updated'; readonly etag: string }
  | { readonly status: 'unchanged'; readonly etag: string }
  | { readonly status: 'failed'; readonly error: unknown };

export type AwaitRefreshResult = RefreshResult | { readonly status: 'timeout' };

export interface RefreshScheduler {
  scheduleRefresh(client: Client): Promise<RefreshResult>;
}

export interface CreateRefreshOptions {
  readonly logger?: TransportLogger;
}

/** Build a refresh scheduler with isolated inflight state. Production callers
 *  should use the default singleton (`scheduleRefresh` export); tests get
 *  fresh state per test by constructing their own. */
export function createRefreshScheduler(options: CreateRefreshOptions = {}): RefreshScheduler {
  const inflight = new Map<string, Promise<RefreshResult>>();
  const logger = options.logger;

  async function performRefresh(client: Client): Promise<RefreshResult> {
    const baseUrl = client.baseUrl;
    try {
      const existing = readCache(baseUrl);
      const result = await listEventTypes(client, { ifNoneMatch: existing?.etag });
      const fetchedAt = new Date().toISOString();

      if ('status' in result) {
        // 304 — defensive guard: server should only 304 in response to a
        // conditional request, but if it does so without a prior cache we
        // have nothing sensible to bump, so we no-op rather than crash.
        if (existing !== null) {
          writeCache(baseUrl, { ...existing, fetchedAt });
        }
        return { status: 'unchanged', etag: result.etag };
      }

      const file: CacheFile = {
        version: 1,
        etag: result.etag,
        fetchedAt,
        serverUrl: baseUrl,
        types: [...result.types],
      };
      writeCache(baseUrl, file);
      return { status: 'updated', etag: result.etag };
    } catch (error) {
      logger?.debug({ event: 'registry.refresh.failed', baseUrl, error });
      return { status: 'failed', error };
    }
  }

  return {
    scheduleRefresh(client: Client): Promise<RefreshResult> {
      const key = client.baseUrl;
      const existing = inflight.get(key);
      if (existing !== undefined) return existing;
      const promise = performRefresh(client).finally(() => {
        inflight.delete(key);
      });
      inflight.set(key, promise);
      return promise;
    },
  };
}

const defaultScheduler = createRefreshScheduler();

/** Default singleton scheduler. Use this for production callers; in tests
 *  prefer `createRefreshScheduler()` to get isolated state. */
export const scheduleRefresh = (client: Client): Promise<RefreshResult> =>
  defaultScheduler.scheduleRefresh(client);

/** Race the refresh promise against a bounded timeout. Returns the refresh
 *  result if it arrives in time, or { status: 'timeout' } otherwise. The
 *  underlying refresh continues running regardless. */
export function awaitRefresh(
  refresh: Promise<RefreshResult>,
  options: { readonly timeoutMs: number },
): Promise<AwaitRefreshResult> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeoutPromise = new Promise<AwaitRefreshResult>((resolve) => {
    timer = setTimeout(() => resolve({ status: 'timeout' }), options.timeoutMs);
  });
  return Promise.race([refresh, timeoutPromise]).finally(() => {
    if (timer !== undefined) clearTimeout(timer);
  });
}
