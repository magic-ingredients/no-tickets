---
id: server-validation-and-storage
prd_id: event-actor-metadata
number: 2
title: Server-side validation gate + DB column
status: not_started
created: 2026-05-14
updated: 2026-05-21
---

# Feature: Server-side validation gate + DB column

## Description

Phase 2 of the opt-in actor-metadata rollout. Lands the database column and the server-side validation gate. `metadata` stays optional on the envelope schema — there is no later phase that flips this — so callers that opt into actor attribution get their actor blocks persisted, and callers that don't keep working unchanged. There is no deprecation window, no metric counting actor-less events as a problem, and no migration of existing rows (they stay `NULL` and that's fine).

When a caller submits valid `metadata`, the server validates it against the canonical schema, persists it to the new `jsonb` column, and surfaces it on read APIs. When the caller omits `metadata`, the server accepts the envelope, stores `NULL`, and that row will never gain a server-synthesised actor.

## Acceptance Criteria

- [ ] `events.metadata` column added as `jsonb` nullable, **permanently** (no later NOT NULL migration)
- [ ] Existing rows are left as-is. No `TRUNCATE`, no backfill — pre-PRD rows stay with `metadata = NULL` forever and that is a valid state.
- [ ] Server validates `metadata` against `eventMetadataSchema` on ingest **when present**; rejects malformed metadata with 422 + structured error
- [ ] Server records `metadata` as `NULL` when callers omit it; does **not** synthesise an actor server-side
- [ ] Read APIs (`/v1/projects/:projectId/feed` etc.) return `metadata` alongside `data` for events that have it, and omit / null the field otherwise
- [ ] All existing event-ingest tests pass; new tests cover validation gate + persistence path for both attributed and unattributed paths
- [ ] Internal publishers (board UI publish actions, MCP server, test fixtures) that *want* actor attribution start passing valid `metadata` blocks; publishers that don't care are left unchanged

## Tasks

### 1. Add `metadata` jsonb column to `events` table
status: not_started

Drizzle migration adding the column. Nullable, permanently. No default value — `NULL` is the explicit signal "this event was published without a declared actor" and is a permanent valid state, not a transitional one.

**Files to modify/create:**
- `packages/notickets-service/src/server/db/schema.ts`
- `packages/notickets-service/drizzle/<timestamp>-add-events-metadata.sql` (new — auto-generated)
- `packages/notickets-service/src/server/db/schema.test.ts`

**Expected changes:**
- New column `metadata` of type `jsonb`, nullable
- Drizzle column metadata + relations updated; type derivation flows to all reads/writes
- Schema test asserts the column exists, type matches, and is nullable
- Migration is reversible (`DROP COLUMN metadata`) — assert in PR description, no test (drizzle migrations don't have automated rollback tests)
- No existing-row migration in this task or any later task. Pre-PRD rows stay with `NULL` metadata.

### 2. Validate `metadata` against the canonical schema on ingest
status: not_started

Server's ingest path runs `eventMetadataSchema.parse()` when `metadata` is present on the envelope. Malformed metadata fails the whole envelope with 422 + structured error naming the offending path. Absent metadata is accepted unconditionally — there is no deprecation tolerance because there is no deprecation.

**Files to modify/create:**
- `packages/notickets-service/src/server/events/ingest.ts`
- `packages/notickets-service/src/server/events/publish-batch.ts`
- `packages/notickets-service/src/server/events/ingest.test.ts`

**Expected changes:**
- Ingest path persists `event.metadata` to the new column when present
- Invalid metadata → 422 with structured error: `{ error: "validation_error", metadataIssues: [{ path, message }] }`
- Test cases: valid metadata roundtrips through to DB; invalid metadata fails 422; absent metadata persists as NULL — and this last case is a **first-class supported path**, not a deprecation tolerance

### 3. Surface `metadata` on read APIs
status: not_started

`/v1/projects/:projectId/feed`, `/v1/events/:id`, and any other event-returning endpoints include `metadata` in the response payload when present, and either omit the field or return `null` when absent (pick one and apply consistently — recommend omit for smaller payloads). No filter params yet; Feature 3 adds those.

**Files to modify/create:**
- `packages/notickets-service/src/server/lib/project-feed-reader.ts`
- `packages/notickets-service/src/server/routes/projects.ts`
- `packages/notickets-service/src/server/routes/events.ts`
- corresponding `.test.ts` files

**Expected changes:**
- Select `metadata` in every event query
- Pass through into JSON response unchanged when present; omit the key when `NULL`
- Tests assert `metadata` appears in the response shape for attributed events and is absent for unattributed ones

### 4. Wire internal publishers to emit metadata where it makes sense
status: not_started

Audit internal publishers and give them actor blocks where attribution is meaningful. Unlike the previous design, this is **opt-in per caller** — not "every internal call must emit." Specifically:

- **Board UI publishes** (state changes from the human-facing board): emit `human` actor from the logged-in user's identity. Already authenticated; no extra surface needed.
- **MCP server publish tool** (`crates/nt-mcp/src/tools/publish.rs`): wire to the same actor resolver as `no-tickets publish`. If the MCP host has declared a session, attribution flows; if not, the tool publishes unattributed (no error).
- **Seed scripts** (`scripts/seed-product-demo.sh`): use `no-tickets session start --agent seed-script` once at the top of the script, then unset on teardown. Demo board events get a `seed-script` pill; the script is fine if no session is set (script-side opt-in).
- **Test fixtures**: leave most fixtures unattributed unless the test specifically exercises actor attribution. Don't bulk-rewrite to "every fixture has a fake actor" — that pollutes the test signal.

No `--model n/a` anywhere. Internal publishers either declare a model (when they have one) or omit the field.

**Files to modify/create:**
- audit pass across `packages/notickets-service/src/client/**` for any `publish(…)` calls
- `crates/nt-mcp/src/tools/publish.rs` — wire to the shared actor resolver from `nt-cli`
- `scripts/seed-product-demo.sh` — add `no-tickets session start --agent seed-script` at top + `session end` at bottom (no `--model` flag)
- selectively update test fixtures that should exercise actor attribution

**Expected changes:**
- Board events from the UI carry a human actor block sourced from the logged-in user
- Seed-script events carry `{ type: 'agent', agentId: 'seed-script' }` — no model field
- MCP publishes carry actor when the MCP host has declared a session; pass through unattributed otherwise
- No internal publisher emits sentinel values like `"n/a"`

## Dependencies

- **Feature 1 (Schemas + no-tickets session)**: must ship first. The schema definition + Rust validator + `no-tickets` CLI wiring are all prerequisites for the server validation gate to mean anything, and for the shared actor resolver that Task 4 reuses.
- **`cross-platform-cli-binary` Task 5 (full MCP server port — ✅ completed 2026-05-20)**: the MCP publish tool exists; this feature adds actor wiring to it.

## Testing Strategy

### Unit Tests

- Drizzle schema test asserts `metadata` column shape (jsonb, nullable)
- Ingest path test: valid metadata persisted; invalid rejected with structured error; absent metadata accepted with NULL persist (first-class path, not tolerated)

### Integration Tests

- Hono request test: POST `/v1/events` with valid metadata → 200 + DB row carries metadata; with invalid metadata → 422 + structured error; without metadata → 200 + DB row carries NULL
- Read-back test: GET `/v1/projects/:id/feed` returns events with `metadata` populated where present, and without the key (or with `null`, per chosen convention) where absent
- Internal publishers: board UI publish → row has human actor metadata; MCP publish without an MCP-host session → row has NULL metadata

### Manual Testing

- Run `scripts/seed-product-demo.sh` against a staging deployment carrying Feature 1 + Feature 2 → confirm seed-script events carry `metadata.actor = { type: 'agent', agentId: 'seed-script' }` (no `model` field)
- Spot-check the staging feed JSON: events from the board carry `metadata.actor`; older events and unattributed publishes do not

## Implementation Notes

- Server does **not** synthesise an actor server-side when one is absent. The hard rule is: actor comes from the client. If we synthesise we lose accountability — what we'd write is "the server saw a missing actor and made one up," which is worse than `NULL`. This is unchanged from the original design and is *why* there's no migration of pre-PRD rows.
- No indexes in this phase. Feature 3 adds partial GIN indexes on the rows that have actor identifiers. They stay partial — the index covers only the opt-in subset of events, which is the whole point.
- Read APIs: pick one convention for absent metadata (omit the key vs `"metadata": null`) and apply consistently. Recommend **omit the key** because clients can `.metadata?.actor?.agentId` either way and omitted-keys are smaller on the wire when most events are unattributed.
- `events.metadata` is a top-level jsonb column, not normalised. Reducer queries reach into it with `metadata->'actor'->>'agentId'`. Index design (Feature 3) supports this access pattern directly.

## Workflow Example

```bash
# Apply the migration
pnpm --filter notickets-service drizzle:migrate

# Confirm column exists
psql "$DATABASE_URL" -c "\d events" | grep metadata

# Submit an attributed event
curl -X POST http://localhost:3000/v1/events \
  -H "Authorization: Bearer $NO_TICKETS_TOKEN" \
  -d '[{"type":"product.feature.created.v1","data":{...},
        "metadata":{"actor":{"type":"agent","agentId":"claude"}},
        "source":{...}}]'
# → 200, row has metadata populated

# Submit an unattributed event
curl -X POST http://localhost:3000/v1/events \
  -H "Authorization: Bearer $NO_TICKETS_TOKEN" \
  -d '[{"type":"product.feature.created.v1","data":{...},
        "source":{...}}]'
# → 200, row has metadata = NULL (permanently valid)
```

## Benefits

- Storage in place for any caller that opts into actor attribution — appears immediately in read APIs
- Validation runs from day one, so malformed metadata is caught immediately rather than silently stored
- No coordinated cutover, no deprecation timer, no team to chase. Adoption is per-caller and per-decision.
- Pre-PRD rows are unaffected — no risk of botched migration, no "Option A vs B" debate
