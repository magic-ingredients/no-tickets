import type { CacheFile } from './cache.js';

export const DEFAULT_STALE_THRESHOLD_DAYS = 14;
const MS_PER_DAY = 24 * 60 * 60 * 1000;

export interface StalenessOptions {
  /** Days threshold. When omitted, falls back to NO_TICKETS_REGISTRY_STALE_DAYS
   *  env var or DEFAULT_STALE_THRESHOLD_DAYS. */
  readonly thresholdDays?: number;
  /** Test seam — defaults to the current wall clock. */
  readonly now?: Date;
}

function thresholdFromEnv(): number {
  const raw = process.env['NO_TICKETS_REGISTRY_STALE_DAYS'];
  if (raw === undefined || raw === '') return DEFAULT_STALE_THRESHOLD_DAYS;
  const parsed = Number(raw);
  if (!Number.isFinite(parsed) || parsed <= 0) return DEFAULT_STALE_THRESHOLD_DAYS;
  return parsed;
}

/** Determine whether the cache is stale relative to the threshold.
 *
 *  Boundary semantics: at exactly `thresholdDays` the cache is fresh;
 *  past that boundary (even by a millisecond) it is stale.
 *
 *  - Null cache → stale (nothing to check).
 *  - Future / unparseable fetchedAt → stale (something is wrong, surface
 *    it rather than mask it). */
export function isCacheStale(
  cache: CacheFile | null,
  options: StalenessOptions = {},
): boolean {
  if (cache === null) return true;

  const fetchedAtMs = Date.parse(cache.fetchedAt);
  if (Number.isNaN(fetchedAtMs)) return true;

  const nowMs = (options.now ?? new Date()).getTime();
  const ageMs = nowMs - fetchedAtMs;
  if (ageMs < 0) return true;

  const thresholdDays = options.thresholdDays ?? thresholdFromEnv();
  return ageMs > thresholdDays * MS_PER_DAY;
}
