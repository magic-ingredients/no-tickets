---
id: registry-introspection
prd_id: client-event-repository-adoption
number: 3
title: Registry Introspection + Caching
status: not_started
created: 2026-04-27
updated: 2026-04-27
---

# Feature: Registry Introspection + Caching

## Description

The discoverability primitive. `client.events.list()` and `client.events.describe(typeId)` against `GET /v1/admin/event-types` and `GET /v1/admin/event-types/:id`. Permission-scoped responses — the listing reflects what the *caller* can write, not the global catalogue. Local cache survives offline reads after the first fetch; ETag-driven refresh keeps it fresh without bytes wasted.

Every higher-level surface (Feature 4 CLI, Feature 5 MCP tools) consumes this. Get it right; everything else is sugar.

### Cache shape

```json
{
  "etag": "W/\"abc123\"",
  "fetchedAt": "2026-04-27T10:23:00Z",
  "types": [
    {
      "id": "engineering.deploy.completed.v1",
      "domain": "engineering",
      "entity": "deploy",
      "action": "completed",
      "version": 1,
      "schema": { /* JSON Schema */ },
      "uiHints": { /* ... */ },
      "retentionDays": 90,
      "dedupeStrategy": "natural_key",
      "deprecatedAt": null
    }
  ]
}
```

### Cache location

`<cwd>/.notickets/.cache/registry.json` — under a hidden `.cache/` subdirectory to avoid colliding with the user's `.notickets/` working data (epics, features, fixes). Cache is per-server-URL: the file key includes a hash of the configured server URL so multi-tenant local development doesn't cross streams.

### Refresh discipline

- Reads always served from cache, synchronously.
- Refresh fires asynchronously on every CLI invocation, not blocking the command.
- Conditional GET with `If-None-Match`. 304 → no-op. 200 → cache replaced atomically (write to temp, rename).
- If refresh fails, log a debug-level note; never error the user-facing command.
- Stale-cache warning when cache age > threshold (default 14 days, configurable) AND last refresh attempt failed. Surfaces via the drift-notification line in routine commands (Feature 4).

## Acceptance Criteria

- [ ] `client.events.list({ domain?, deprecated? })` returns cached entries; refreshes async.
- [ ] `client.events.describe(typeId)` returns one type from cache; falls back to `/v1/admin/event-types/:id` on cache miss.
- [ ] First fetch from clean state populates cache and returns within 2s on a healthy network.
- [ ] Subsequent calls offline succeed if cache exists; fail with a clear diagnostic if cache is missing.
- [ ] Permission scoping: server-side filter is reflected in the cached payload (we do not re-filter client-side).
- [ ] ETag respected: `If-None-Match` sent on refresh; 304 leaves cache untouched but updates `fetchedAt`.
- [ ] Cache file is valid JSON; corrupt cache wipes-and-refetches without erroring the calling command.
- [ ] Cache namespaced per server URL.

## Tasks

### 1. Registry HTTP client (list + describe)

**Files to modify/create:**
- `src/registry/client.ts` (new)
- `src/registry/client.test.ts` (new)

**Expected changes:**
- `listEventTypes(client, { domain?, deprecated?, ifNoneMatch? }): Promise<{ etag, types } | { etag, status: 304 }>`.
- `getEventType(client, id): Promise<EventTypeSpec | null>`.
- Tests cover query param shaping, 304 handling, permission-filtered responses.

### 2. Cache layer

**Files to modify/create:**
- `src/registry/cache.ts` (new)
- `src/registry/cache.test.ts` (new)

**Expected changes:**
- `readCache(serverUrl): CacheFile | null` — corrupt cache → null, not throw.
- `writeCache(serverUrl, file)` — atomic temp+rename.
- `cachePath(serverUrl)` — hashed-key file under `.notickets/.cache/registry-<hash>.json`.
- Tests: corrupt cache recovery, atomic write, multi-server isolation.

### 3. `client.events.list` + `client.events.describe`

**Files to modify/create:**
- `src/registry/index.ts` (new — public surface)
- `src/registry/index.test.ts` (new)
- `src/index.ts` (export)

**Expected changes:**
- `events.list({ domain?, deprecated? })` returns from cache; triggers async refresh.
- `events.describe(typeId)` looks in cache, falls back to one-shot fetch, populates cache on hit.
- Tests cover: cache-only mode, offline-with-cache, cache-miss-network-fallback, refresh failure doesn't break read.

### 4. Async refresh worker

**Files to modify/create:**
- `src/registry/refresh.ts` (new)
- `src/registry/refresh.test.ts` (new)

**Expected changes:**
- `scheduleRefresh(client)` fires-and-forgets; bounded concurrency (one inflight refresh per server).
- Refresh uses cached ETag; merges 200 atomically, leaves cache on 304.
- Failures logged at debug level; success returns the new state for callers that want it.
- Tests: parallel calls coalesce, failed refresh leaves prior cache, 304 updates `fetchedAt` only.

### 5. Stale-cache detection

**Files to modify/create:**
- `src/registry/staleness.ts` (new)
- `src/registry/staleness.test.ts` (new)

**Expected changes:**
- `isCacheStale(cache, { thresholdDays })` returns boolean.
- Threshold defaults to 14, configurable via env (`NO_TICKETS_REGISTRY_STALE_DAYS`) or settings.
- Tests cover boundary conditions, missing cache treated as stale.

## Dependencies

- Feature 1 (envelope schemas) — type definitions for cache contents.
- Feature 2 (HTTP client) — auth-aware client to make introspection calls.
- Server-side `event-repository-foundation` Feature 1 Task 6 — introspection route.

## Testing Strategy

### Unit Tests
- Cache read/write atomicity.
- ETag round-trip behaviour.
- Permission scoping reflected verbatim from server response.

### Integration Tests
- Full `events.list` flow against a fixture server: first call fetches, second call cached, refresh after change in registry.
- Offline mode after cache populated.
- Multi-server URL cache isolation.
