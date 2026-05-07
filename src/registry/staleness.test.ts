import { describe, it, expect, afterEach } from 'vitest';
import { isCacheStale, DEFAULT_STALE_THRESHOLD_DAYS } from './staleness.js';
import type { CacheFile } from './cache.js';
import type { EventTypeSpec } from './client.js';

const TYPE_A: EventTypeSpec = {
  id: 'app.user.signed-up.v1',
  domain: 'app.user',
  entity: 'user',
  action: 'signed-up',
  version: 1,
  schema: { type: 'object', properties: {} },
};

const NOW = new Date('2026-05-07T00:00:00Z');

function buildCache(fetchedAt: string): CacheFile {
  return {
    version: 1,
    etag: 'W/"x"',
    fetchedAt,
    serverUrl: 'https://api.example.com',
    types: [TYPE_A],
  };
}

afterEach(() => {
  delete process.env['NO_TICKETS_REGISTRY_STALE_DAYS'];
});

describe('isCacheStale — null cache', () => {
  it('treats a null cache as stale', () => {
    expect(isCacheStale(null, { now: NOW })).toBe(true);
  });
});

describe('isCacheStale — explicit threshold', () => {
  it('returns false when the cache is younger than the threshold', () => {
    const cache = buildCache('2026-05-01T00:00:00Z'); // 6 days ago
    expect(isCacheStale(cache, { thresholdDays: 14, now: NOW })).toBe(false);
  });

  it('returns true when the cache is older than the threshold', () => {
    const cache = buildCache('2026-04-01T00:00:00Z'); // 36 days ago
    expect(isCacheStale(cache, { thresholdDays: 14, now: NOW })).toBe(true);
  });

  it('returns false at the exact threshold boundary (== threshold is fresh; > threshold is stale)', () => {
    // Cache fetched exactly 14 days before NOW. Boundary semantics: at the
    // threshold the cache is fresh; one ms past makes it stale (next test).
    const cache = buildCache('2026-04-23T00:00:00Z');
    expect(isCacheStale(cache, { thresholdDays: 14, now: NOW })).toBe(false);
  });

  it('returns true one millisecond past the threshold', () => {
    const cache = buildCache('2026-04-22T23:59:59.999Z');
    expect(isCacheStale(cache, { thresholdDays: 14, now: NOW })).toBe(true);
  });

  it('treats age 0 (fetchedAt === now) as fresh (boundary kills the `<` → `<=` mutation)', () => {
    const cache = buildCache(NOW.toISOString());
    expect(isCacheStale(cache, { thresholdDays: 14, now: NOW })).toBe(false);
  });

  it('returns true when the cache fetchedAt is in the future (clock skew)', () => {
    // Future timestamps mean negative age; treat as stale (something's wrong).
    const cache = buildCache('2030-01-01T00:00:00Z');
    expect(isCacheStale(cache, { thresholdDays: 14, now: NOW })).toBe(true);
  });

  it('treats an unparseable fetchedAt as stale', () => {
    const cache = buildCache('not-a-date');
    expect(isCacheStale(cache, { thresholdDays: 14, now: NOW })).toBe(true);
  });
});

describe('isCacheStale — default threshold', () => {
  it('exposes 14 days as the default threshold (PRD)', () => {
    expect(DEFAULT_STALE_THRESHOLD_DAYS).toBe(14);
  });

  it('uses 14 days when no threshold is supplied', () => {
    const fresh = buildCache('2026-05-01T00:00:00Z'); // 6 days
    const old = buildCache('2026-04-01T00:00:00Z'); // 36 days

    expect(isCacheStale(fresh, { now: NOW })).toBe(false);
    expect(isCacheStale(old, { now: NOW })).toBe(true);
  });
});

describe('isCacheStale — env override', () => {
  it('honours NO_TICKETS_REGISTRY_STALE_DAYS when no explicit threshold is supplied', () => {
    process.env['NO_TICKETS_REGISTRY_STALE_DAYS'] = '3';
    const cache = buildCache('2026-05-01T00:00:00Z'); // 6 days

    expect(isCacheStale(cache, { now: NOW })).toBe(true);
  });

  it('explicit thresholdDays beats the env var', () => {
    process.env['NO_TICKETS_REGISTRY_STALE_DAYS'] = '3';
    const cache = buildCache('2026-05-01T00:00:00Z'); // 6 days

    expect(isCacheStale(cache, { thresholdDays: 14, now: NOW })).toBe(false);
  });

  it('ignores a non-numeric env var (falls back to default)', () => {
    process.env['NO_TICKETS_REGISTRY_STALE_DAYS'] = 'not-a-number';
    const cache = buildCache('2026-05-01T00:00:00Z'); // 6 days

    expect(isCacheStale(cache, { now: NOW })).toBe(false);
  });

  it('ignores a non-positive env var (falls back to default)', () => {
    process.env['NO_TICKETS_REGISTRY_STALE_DAYS'] = '0';
    const cache = buildCache('2026-05-01T00:00:00Z');

    expect(isCacheStale(cache, { now: NOW })).toBe(false);
  });

  it('treats an empty-string env var as unset (falls back to default)', () => {
    process.env['NO_TICKETS_REGISTRY_STALE_DAYS'] = '';
    const cache = buildCache('2026-05-01T00:00:00Z'); // 6 days

    expect(isCacheStale(cache, { now: NOW })).toBe(false);
  });

  it('treats an unset env var distinctly from an empty-string env var (kills `||` mutations)', () => {
    // Explicitly delete (env var truly absent — `process.env[X]` is undefined,
    // not `""`). The `raw === undefined` branch must short-circuit before the
    // `raw === ''` check evaluates.
    delete process.env['NO_TICKETS_REGISTRY_STALE_DAYS'];
    const fresh = buildCache('2026-05-01T00:00:00Z'); // 6 days
    const old = buildCache('2026-04-01T00:00:00Z'); // 36 days

    expect(isCacheStale(fresh, { now: NOW })).toBe(false);
    expect(isCacheStale(old, { now: NOW })).toBe(true);
  });

  it('explicit thresholdDays: undefined falls through to env (kills the `!== undefined` guard mutation)', () => {
    process.env['NO_TICKETS_REGISTRY_STALE_DAYS'] = '3';
    const cache = buildCache('2026-05-01T00:00:00Z'); // 6 days

    // Without the `!== undefined` guard, `isValidThreshold(undefined)` would
    // be called on undefined and short-circuit the env path.
    expect(isCacheStale(cache, { thresholdDays: undefined, now: NOW })).toBe(true);
  });
});

describe('isCacheStale — explicit threshold validation', () => {
  it('falls back to env / default when explicit thresholdDays is NaN', () => {
    process.env['NO_TICKETS_REGISTRY_STALE_DAYS'] = '3';
    const cache = buildCache('2026-05-01T00:00:00Z'); // 6 days
    // Explicit NaN is invalid, so env (3 days) takes over → cache is stale.
    expect(isCacheStale(cache, { thresholdDays: NaN, now: NOW })).toBe(true);
  });

  it('falls back to env / default when explicit thresholdDays is non-positive', () => {
    process.env['NO_TICKETS_REGISTRY_STALE_DAYS'] = '3';
    const cache = buildCache('2026-05-01T00:00:00Z'); // 6 days
    expect(isCacheStale(cache, { thresholdDays: -5, now: NOW })).toBe(true);
    expect(isCacheStale(cache, { thresholdDays: 0, now: NOW })).toBe(true);
  });

  it('falls back to default when explicit thresholdDays is Infinity', () => {
    // Infinity * MS_PER_DAY is Infinity; ageMs > Infinity is always false,
    // so without validation everything would be "fresh" forever — masking
    // an obviously bad caller value.
    const cache = buildCache('2020-01-01T00:00:00Z'); // very old
    expect(isCacheStale(cache, { thresholdDays: Infinity, now: NOW })).toBe(true);
  });
});
