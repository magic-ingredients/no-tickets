import type { Client } from '../transport/client.js';
import { readCache, writeCache, type CacheFile } from './cache.js';
import { getEventType, type EventTypeSpec } from './client.js';

export interface EventsListOptions {
  readonly domain?: string;
  readonly deprecated?: boolean;
}

export interface EventsApi {
  list(options?: EventsListOptions): Promise<readonly EventTypeSpec[]>;
  describe(typeId: string): Promise<EventTypeSpec | null>;
}

export interface CreateEventsDeps {
  readonly client: Client;
  /** Fire-and-forget refresh scheduler. When omitted, list() reads cache
   *  without scheduling. Task 3-4 wires the real implementation. */
  readonly scheduleRefresh?: (client: Client) => Promise<unknown>;
}

const CACHE_MISSING_MESSAGE =
  'Registry cache not found. Populate it with `nt event list` or wait for first refresh.';

function isDeprecated(type: EventTypeSpec): boolean {
  return type.deprecatedAt !== null && type.deprecatedAt !== undefined;
}

function applyFilters(
  types: readonly EventTypeSpec[],
  options: EventsListOptions,
): readonly EventTypeSpec[] {
  let filtered: readonly EventTypeSpec[] = types;
  if (options.domain !== undefined) {
    filtered = filtered.filter((t) => t.domain === options.domain);
  }
  if (options.deprecated !== undefined) {
    filtered = filtered.filter((t) => isDeprecated(t) === options.deprecated);
  }
  return filtered;
}

function fireAndForget(
  scheduleRefresh: ((client: Client) => Promise<unknown>) | undefined,
  client: Client,
): void {
  if (scheduleRefresh === undefined) return;
  // Refresh failures must not surface to the read path. Always swallow.
  Promise.resolve()
    .then(() => scheduleRefresh(client))
    .catch(() => {
      // intentional: PRD says refresh failures log at debug level only.
    });
}

export function createEvents(deps: CreateEventsDeps): EventsApi {
  const { client, scheduleRefresh } = deps;

  return {
    async list(options: EventsListOptions = {}): Promise<readonly EventTypeSpec[]> {
      const cache = readCache(client.baseUrl);
      if (cache === null) throw new Error(CACHE_MISSING_MESSAGE);

      fireAndForget(scheduleRefresh, client);
      return applyFilters(cache.types, options);
    },

    async describe(typeId: string): Promise<EventTypeSpec | null> {
      const cache = readCache(client.baseUrl);
      const cached = cache?.types.find((t) => t.id === typeId);
      if (cached !== undefined) return cached;

      const fresh = await getEventType(client, typeId);
      if (fresh !== null && cache !== null) {
        const updated: CacheFile = {
          ...cache,
          types: [...cache.types, fresh],
        };
        writeCache(client.baseUrl, updated);
      }
      return fresh;
    },
  };
}
