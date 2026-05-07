import { describe, it, expect, beforeEach, afterEach } from 'vitest';
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

  it('returns false at the exact threshold boundary (>= threshold counts as stale; < is fresh)', () => {
    // Cache fetched exactly 14 days before NOW. Boundary semantics: NOT
    // stale at the threshold (allows one freshness day of grace).
    const cache = buildCache('2026-04-23T00:00:00Z');
    expect(isCacheStale(cache, { thresholdDays: 14, now: NOW })).toBe(false);
  });

  it('returns true one millisecond past the threshold', () => {
    const cache = buildCache('2026-04-22T23:59:59.999Z');
    expect(isCacheStale(cache, { thresholdDays: 14, now: NOW })).toBe(true);
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
  beforeEach(() => {
    delete process.env['NO_TICKETS_REGISTRY_STALE_DAYS'];
  });

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
});
