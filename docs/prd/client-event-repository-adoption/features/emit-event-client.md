---
id: emit-event-client
prd_id: client-event-repository-adoption
number: 2
title: publish() + Subjects + Interactions HTTP Client
status: not_started
created: 2026-04-27
updated: 2026-05-06
---

# Feature: `publish()` + Subjects + Interactions HTTP Client

## Description

The transport surface. One TS module that speaks the wire protocol against the server's event repository: `publish`, `subjects.create/list/get`, `runInteraction`. Replaces the existing `push` HTTP client. `data` and `input` are pass-through — the SDK does not validate domain payloads; the server does.

`publish` always takes an `Event[]` array — single event is `[oneEvent]`, batch is `[a, b, c, …]`. The wire body is the same array, no envelope wrapper. This matches the server-side `single-events-endpoint-and-product-domain` PRD's `POST /v1/events` shape.

This feature also removes the old `push` command/CLI surface that depended on the removed schemas (Feature 1 deleted the schemas; this feature deletes the consumers).

### Wire protocol

| Operation        | Method | Path                       | Body                                                      |
|------------------|--------|----------------------------|-----------------------------------------------------------|
| Publish events   | POST   | `/v1/events`               | `Event[]` array (single event = `[oneEvent]`)             |
| List subjects    | GET    | `/v1/subjects?type=...`    | —                                                         |
| Get subject      | GET    | `/v1/subjects/:type/:id`   | —                                                         |
| Promote subject  | POST   | `/v1/subjects`             | `Subject`                                                 |
| Run interaction  | POST   | `/v1/interactions/:id`     | `{ input, subject? }`                                     |

The `POST /v1/events` shape is owned by the server's `single-events-endpoint-and-product-domain` PRD; this feature conforms to it.

### Auth

Existing auth (Feature 1 of `no-tickets-client`) is reused unchanged. Push tokens (`NO_TICKETS_TOKEN`) and session tokens (`~/.notickets/credentials`) both work; resolver order unchanged.

### Error mapping

The single HTTP module maps server errors to typed exceptions:
- 422 unknown type → `UnknownEventTypeError(typeId, batchIndex)` — server reports the index of the bad entry within the batch
- 422 schema mismatch → `EventValidationError(typeId, issues, batchIndex)`
- 403 → `PermissionDeniedError(domain)`
- 5xx → `ServerError(status, body)` with bounded retries (idempotent ops only)

Per-event errors fail the whole batch (the server runs everything in one transaction). The error carries the failing index so callers can identify which event in their batch was the cause.

## Acceptance Criteria

- [ ] `publish(events: Event[])` POSTs to `/v1/events`, returns `{ ingested, deduped, ids }`.
- [ ] Single-event convenience: `publish([oneEvent])` works without any wrapper.
- [ ] Batch publish: `publish([a, b, c])` sends one request with all three events.
- [ ] Per-event validation error from the server returns a typed exception with the batch index of the failing event.
- [ ] `subjects.create(subject)`, `subjects.get(ref)`, `subjects.list({ type })` round-trip.
- [ ] `runInteraction(id, { input, subject? })` returns the server's response (`{ events }` or final shape).
- [ ] Auth resolution unchanged; push tokens and session tokens both work.
- [ ] Typed errors thrown for 4xx; bounded retries for 5xx on idempotent calls.
- [ ] Old `push` command and HTTP client are removed (consumer deletion of Feature 1's schema deletion).
- [ ] tiny-brain switched off the push payload — publishes `ai.completion.recorded.v1`, `ai.review.completed.v1`, `ai.task.completed.v1` directly via `publish`.

## Tasks

### 1. HTTP client core
End-to-end task — failing tests, implementation, and any review-driven refactors land here.

**Files to modify/create:**
- `src/transport/client.ts` (new — replaces existing push client)
- `src/transport/client.test.ts` (new)
- `src/transport/errors.ts` (new)
- `src/transport/errors.test.ts` (new)

**Expected changes:**
- Single `Client` class accepting `{ baseUrl, token, fetch? }`.
- Methods: `request(method, path, body?)` is the private workhorse; per-operation wrappers below.
- Error mapping centralised here; typed exception classes exported.
- Tests cover auth header injection, retry logic on 5xx, error mapping for each documented status, batch-index propagation on per-event 422.

### 2. publish (array body)
End-to-end task — failing tests, implementation, and any review-driven refactors land here.

**Files to modify/create:**
- `src/transport/events.ts` (new)
- `src/transport/events.test.ts` (new)
- `src/index.ts` (export)

**Expected changes:**
- `publish(client, events: Event[]): Promise<{ ingested, deduped, ids }>`.
- Validates each envelope locally with `eventSchema` before sending (cheap fail-fast); aborts on first invalid envelope and reports its index.
- Sends as a single `POST /v1/events` with the array as the JSON body — no wrapper key.
- Tests cover happy path single + batch, schema fail before send carries index, server-side 422 unknown type carries the server's batch index, dedupe count matches the response.

### 3. Subjects API
End-to-end task — failing tests, implementation, and any review-driven refactors land here.

**Files to modify/create:**
- `src/transport/subjects.ts` (new)
- `src/transport/subjects.test.ts` (new)
- `src/index.ts`

**Expected changes:**
- `subjects.create(client, subject)`, `subjects.get(client, ref)`, `subjects.list(client, query)`.
- Tests cover CRUD round-trip, filter validation, 404 mapping.

### 4. Interactions API
End-to-end task — failing tests, implementation, and any review-driven refactors land here.

**Files to modify/create:**
- `src/transport/interactions.ts` (new)
- `src/transport/interactions.test.ts` (new)
- `src/index.ts`

**Expected changes:**
- `runInteraction(client, id, { input, subject? })`.
- Validates request envelope locally before send.
- Returns server response unchanged (typed against `interactionResponseSchema`).
- Tests cover happy path, permission denial, validation error from server.

### 5. Remove push command + push HTTP client
End-to-end task — failing tests, implementation, and any review-driven refactors land here.

**Files to modify/create:**
- `src/commands/push.ts` (delete)
- `src/commands/push.test.ts` (delete)
- `src/sdk/api-client.ts` — drop `client.push()` and the `Push`-typed signature
- `src/cli.ts` — remove `push` subcommand registration
- `src/cli.test.ts`
- `bin/no-tickets` — verify no dangling reference

**Expected changes:**
- `npx no-tickets push` no longer exists.
- CLI exit code unchanged for bad subcommands (helpful error).
- Tests assert push is not in the help listing and exits non-zero with a hint to use `nt publish` (Feature 4 lands the new command; in the interim, the message stands alone).

### 6. tiny-brain integration cutover
End-to-end task — failing tests, implementation, and any review-driven refactors land here.

**Files to modify/create (in tiny-brain repo, tracked here for completeness):**
- tiny-brain's push integration

**Expected changes:**
- tiny-brain publishes `ai.completion.recorded.v1`, `ai.review.completed.v1`, and `ai.task.completed.v1` directly via the SDK's `publish`, not via the legacy push payload.
- Session-end batches use a single `publish([...])` call, not N round-trips.
- Validation of the cutover happens in tiny-brain's CI; this feature ships the SDK surface tiny-brain consumes.

This task is tracked in this feature for visibility; the implementation lives in tiny-brain.

## Dependencies

- Feature 1 (envelope schemas) of this PRD — `eventSchema` etc. exist before this feature wires them.
- Server-side `single-events-endpoint-and-product-domain` PRD Feature 1 — `POST /v1/events` (array body) endpoint must exist.
- Server-side `event-repository-foundation` Features 1, 2 — `/v1/subjects`, `/v1/interactions` endpoints must exist.
- `domain-ai-telemetry` Feature 1 — registered `ai.*` event types so tiny-brain has somewhere real to publish.

## Testing Strategy

### Unit Tests
- Each transport method isolated against a mock fetch.
- Error mapping per documented status.
- Batch-index propagation on per-event 422.
- Auth header injection across token sources.

### Integration Tests
- `nt publish` and `nt action` (added in Feature 4) round-trip against a real server fixture; this feature's tests cover the underlying client.
- Cutover test in tiny-brain proves end-to-end the legacy push code path is dead.
