---
id: registry-introspection
prd_id: client-event-repository-adoption
number: 3
title: Registry Introspection + Caching
status: completed
created: 2026-04-27
updated: 2026-05-17
---

# Feature: Registry Introspection + Caching

## Description

The discoverability primitive. `client.events.list()` and `client.events.describe(typeId)` against `GET /v1/admin/event-types` and `GET /v1/admin/event-types/:id`. Permission-scoped responses — the listing reflects what the *caller* can write, not the global catalogue. Local cache survives offline reads after the first fetch; ETag-driven refresh keeps it fresh without bytes wasted.

Every higher-level surface (Feature 4 CLI, Feature 5 MCP tools) consumes this. Get it right; everything else is sugar.

### Cache shape

```json
{
  "version": 1,
  "etag": "W/\"abc123\"",
  "fetchedAt": "2026-04-27T10:23:00Z",
  "serverUrl": "https://no-tickets.example.com",
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

`version: 1` is required upfront so future shape migrations have a clean discriminator. Future cache shapes bump `version` and the reader either upgrades in place or wipes-and-refetches.

### Cache location

Resolution order:
1. **Project-local**: `<cwd>/.notickets/.cache/registry-<hash>.json` if `<cwd>/.notickets/` exists or any ancestor up to git root contains one.
2. **User-local fallback**: `~/.notickets/.cache/registry-<hash>.json` if no project-local `.notickets/` is found.

Under a hidden `.cache/` subdirectory to avoid colliding with the user's `.notickets/` working data (epics, features, fixes). Cache is per-server-URL: the file key (`<hash>`) is a hash of the configured server URL so multi-tenant local development doesn't cross streams.

The README documents that `.notickets/.cache/` should be gitignored when `.notickets/` itself is committed.

### Refresh discipline

- Reads always served from cache, synchronously.
- Refresh fires asynchronously on every CLI invocation. Commands wait briefly (default 200ms, configurable via `NO_TICKETS_REFRESH_WAIT_MS`) for the refresh to complete before computing drift; if it doesn't complete in time, the command proceeds without blocking and the drift notification is deferred to the next invocation.
- Conditional GET with `If-None-Match`. 304 → no-op. 200 → cache replaced atomically (write to temp, rename).
- If refresh fails, log a debug-level note; never error the user-facing command.
- Stale-cache warning when cache age > threshold (default 14 days, configurable) AND last refresh attempt failed. Surfaces via the drift-notification line in routine commands (Feature 4).
- Multi-process race: two concurrent invocations may both fire refreshes. Atomic temp+rename prevents file corruption; ETag mostly mitigates duplicate work; the rare case where a slower-but-newer 200 loses to a faster-but-older 200 is documented behaviour (next refresh self-heals via ETag).

### Auth scoping

`/v1/admin/event-types` accepts both push tokens and session tokens. Required so CI runners with only push tokens can validate locally. Confirmed contract with the server PRD (`event-repository-foundation` Feature 1 Task 6).

## Acceptance Criteria

- [ ] `client.events.list({ domain?, deprecated? })` returns cached entries; refreshes async; bounded wait on refresh (default 200ms) before returning so first-invocation drift detection works.
- [ ] `client.events.describe(typeId)` returns one type from cache; falls back to `/v1/admin/event-types/:id` on cache miss.
- [ ] First fetch from clean state populates cache and returns within 2s on a healthy network.
- [ ] Subsequent calls offline succeed if cache exists; fail with a clear diagnostic if cache is missing.
- [ ] Permission scoping: server-side filter is reflected in the cached payload (we do not re-filter client-side).
- [ ] ETag respected: `If-None-Match` sent on refresh; 304 leaves cache untouched but updates `fetchedAt`.
- [ ] Cache file is valid JSON, carries `"version": 1`; corrupt or unknown-version cache wipes-and-refetches without erroring the calling command.
- [ ] Cache namespaced per server URL; project-local location preferred, user-local fallback when no `.notickets/` in cwd or ancestors.
- [ ] Both push tokens and session tokens accepted by `/v1/admin/event-types` (CI runners with push tokens can introspect).

## Tasks

### 1. Registry HTTP client (list + describe)

status: completed
commitSha: f48d34a

**Reconciliation (2026-05-17):** `src/registry/client.ts` ships `listEventTypes` + `getEventType` against `/v1/admin/event-types` (later migrated to `/v1/registry/event-types` per commit `d69b851`). 341-LOC test in `client.test.ts`. Parallel work in Rust at `crates/nt-mcp/src/registry_cache.rs` for the MCP-side caching path (commit `e7e6b73`).

**Files to modify/create:**
- `src/registry/client.ts` (new)
- `src/registry/client.test.ts` (new)

**Expected changes:**
- `listEventTypes(client, { domain?, deprecated?, ifNoneMatch? }): Promise<{ etag, types } | { etag, status: 304 }>`.
- `getEventType(client, id): Promise<EventTypeSpec | null>`.
- Tests cover query param shaping, 304 handling, permission-filtered responses.

### 2. Cache layer

status: completed
commitSha: 101e6b4

**Reconciliation (2026-05-17):** `src/registry/cache.ts` ships read/write with atomic temp+rename, version-discrimination, and the cwd-ancestor walk for `.notickets/.cache/` resolution. Cache path defaults documented in `docs/prd/client-event-repository-adoption/prd.md` line 295.

**Files to modify/create:**
- `src/registry/cache.ts` (new)
- `src/registry/cache.test.ts` (new)

**Expected changes:**
- `readCache(serverUrl): CacheFile | null` — corrupt cache or unknown `version` → null, not throw.
- `writeCache(serverUrl, file)` — atomic temp+rename. `file` carries `version: 1`.
- `cachePath(serverUrl)`: walks from cwd up to git root looking for `.notickets/`; returns `<found>/.notickets/.cache/registry-<hash>.json` if found, otherwise `~/.notickets/.cache/registry-<hash>.json`.
- Tests: corrupt cache recovery, unknown-version recovery, atomic write, multi-server isolation, project-local preferred over user-local, fallback to user-local when no `.notickets/` exists in cwd ancestors.

### 3. `client.events.list` + `client.events.describe`

status: completed
commitSha: 41aef25

**Reconciliation (2026-05-17):** `src/registry/index.ts` ships the `events.list` / `events.describe` facade with cache fallback + async refresh. 368-LOC test in `index.test.ts`. `src/index.ts` (export) not created — same npm-export gap as publish-client tasks 1–4 (resolution gated on `cross-platform-cli-binary` Task 33, TS-SDK Phase 4 decision).

**Files to modify/create:**
- `src/registry/index.ts` (new — public surface)
- `src/registry/index.test.ts` (new)
- `src/index.ts` (export)

**Expected changes:**
- `events.list({ domain?, deprecated? })` returns from cache; triggers async refresh.
- `events.describe(typeId)` looks in cache, falls back to one-shot fetch, populates cache on hit.
- Tests cover: cache-only mode, offline-with-cache, cache-miss-network-fallback, refresh failure doesn't break read.

### 4. Async refresh worker

status: completed
commitSha: 3c13733

**Reconciliation (2026-05-17):** `src/registry/refresh.ts` ships `scheduleRefresh` + `awaitRefresh` + bounded per-server-URL concurrency. 358-LOC test in `refresh.test.ts`.

**Files to modify/create:**
- `src/registry/refresh.ts` (new)
- `src/registry/refresh.test.ts` (new)

**Expected changes:**
- `scheduleRefresh(client): Promise<RefreshResult>` — fires async, returns a promise callers can optionally await with a bounded timeout.
- `awaitRefresh(promise, { timeoutMs })` — utility for callers (CLI commands) that want to wait briefly before computing drift.
- Bounded concurrency (one inflight refresh per server URL within a process; cross-process race documented as accepted).
- Refresh uses cached ETag; merges 200 atomically, leaves cache on 304.
- Failures logged at debug level; success returns the new state for callers that want it.
- Tests: parallel in-process calls coalesce, failed refresh leaves prior cache, 304 updates `fetchedAt` only, `awaitRefresh` returns within timeoutMs even if refresh is slow.

### 5. Stale-cache detection

status: completed
commitSha: 6aa41ed

**Reconciliation (2026-05-17):** `src/registry/staleness.ts` ships `isCacheStale` with 14-day default and env override (`NO_TICKETS_REGISTRY_STALE_DAYS`). 171-LOC test in `staleness.test.ts`.

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
