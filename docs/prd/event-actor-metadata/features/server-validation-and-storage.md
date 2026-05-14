---
id: server-validation-and-storage
prd_id: event-actor-metadata
number: 2
title: Server-side validation gate + DB column + backfill
status: not_started
created: 2026-05-14
updated: 2026-05-14
---

# Feature: Server-side validation gate + DB column + backfill

## Description

Phase 2 of the actor-metadata rollout. Lands the database column, the server-side validation gate, and the migration strategy for existing rows. `metadata` is still optional on the envelope schema in this phase (Phase 3 flips it to required) so callers that haven't yet adopted `nt session` keep working through the deprecation window — but every event that *does* carry metadata is validated against the canonical schema, and the column is in place to make Phase 3's hard cutover atomic.

A Prometheus-style metric `events_ingested_without_metadata_total` is emitted during this phase. The metric is the gate for Phase 3: when it drops to ~zero across all known callers for a full week, we ship Phase 3.

## Acceptance Criteria

- [ ] `events.metadata` column added as `jsonb` nullable (or with empty-object default per the migration strategy below)
- [ ] Migration is reversible — `DROP COLUMN metadata` remains a safe rollback path until Phase 3 cutover
- [ ] Decision applied per §10 of the PRD: either truncate existing rows in non-prod environments, or backfill with a `system`-style placeholder (requires `system` variant to be added in this phase if backfill is chosen)
- [ ] Server validates `metadata` against `eventMetadataSchema` on ingest when present; rejects malformed metadata with 422 + structured error
- [ ] Server records `metadata` as `NULL` (or empty placeholder) when callers omit it; does not synthesise an actor server-side
- [ ] Metric `events_ingested_without_metadata_total` increments per actor-less event, labelled by project id and event type
- [ ] Read APIs (`/v1/projects/:projectId/feed` etc.) return `metadata` alongside `data` for events that have it
- [ ] All existing event-ingest tests pass; new tests cover validation gate + persistence path
- [ ] Internal callers (`nt-cli`, MCP server, board UI publishes) emit metadata for 100% of events in staging by end of Phase 2

## Tasks

### 1. Add `metadata` jsonb column to `events` table
status: not_started

Drizzle migration adding the column. Nullable in this phase. No default value — `NULL` is the explicit signal "this event predates the actor model" and is what the metric counts. (Alternative: default to `'{}'::jsonb` to simplify NOT NULL transition later. Decide and document in the migration's comment.)

**Files to modify/create:**
- `packages/notickets-service/src/server/db/schema.ts`
- `packages/notickets-service/drizzle/<timestamp>-add-events-metadata.sql` (new — auto-generated)
- `packages/notickets-service/src/server/db/schema.test.ts`

**Expected changes:**
- New column `metadata` of type `jsonb`, nullable
- Drizzle column metadata + relations updated; type derivation flows to all reads/writes
- Schema test asserts the column exists, type matches, and is nullable
- Migration is reversible (`DROP COLUMN metadata`) — assert in PR description, no test (drizzle migrations don't have automated rollback tests)

### 2. Apply migration strategy for existing rows
status: not_started

Per §10 of the PRD, two options. Decide and apply:

**Option A (recommended for staging / non-prod):** `TRUNCATE events;` in the migration. Aligns with the project's no-v1-backcompat policy. Demo data regenerates from `scripts/seed-product-demo.sh` in seconds.

**Option B (only if a prod environment carries irreplaceable data):** backfill `metadata = '{"actor":{"type":"system","systemId":"pre-actor-migration"}}'::jsonb` for every existing row. Requires adding the `system` actor variant to the v1 schema in Feature 1 — pull that change forward if Option B is chosen.

**Files to modify/create:**
- `packages/notickets-service/drizzle/<timestamp>-events-metadata-backfill.sql` (new — separate migration step from column add, so a rollback of the backfill doesn't touch the schema)
- Migration runbook entry in `docs/operations/migrations.md`

**Expected changes:**
- One of: TRUNCATE (Option A) or UPDATE with the placeholder (Option B)
- Migration runs in a single transaction
- Runbook notes the rollback path (Option A is irreversible by definition; Option B is reversible by setting the column back to NULL)

### 3. Validate `metadata` against the canonical schema on ingest
status: not_started

Server's ingest path runs `eventMetadataSchema.parse()` when `metadata` is present on the envelope. Malformed metadata fails the whole envelope with 422 + structured error naming the offending path. Absent metadata is accepted (Phase 1 tolerance still applies).

**Files to modify/create:**
- `packages/notickets-service/src/server/events/ingest.ts`
- `packages/notickets-service/src/server/events/publish-batch.ts`
- `packages/notickets-service/src/server/events/ingest.test.ts`

**Expected changes:**
- Ingest path persists `event.metadata` to the new column when present
- Invalid metadata → 422 with structured error: `{ error: "validation_error", metadataIssues: [{ path, message }] }`
- Test cases: valid metadata roundtrips through to DB; invalid metadata fails 422; absent metadata persists as NULL

### 4. Emit `events_ingested_without_metadata_total` metric
status: not_started

Prometheus-style counter for the deprecation window. Labelled by `projectId`, `eventType`, and `actorTypeHint` (which is `"none"` for actor-less events; useful when we add multiple "missing" reasons later). The metric drives the decision to ship Phase 3.

**Files to modify/create:**
- `packages/notickets-service/src/server/observability/metrics.ts`
- `packages/notickets-service/src/server/events/ingest.ts`
- `packages/notickets-service/src/server/observability/metrics.test.ts`

**Expected changes:**
- Counter incremented in ingest path when `event.metadata` is absent
- Labels documented in `docs/operations/metrics.md`
- Dashboard panel added (Grafana JSON) tracking the metric per project — manual import for now

### 5. Surface `metadata` on read APIs
status: not_started

`/v1/projects/:projectId/feed`, `/v1/events/:id`, and any other event-returning endpoints include `metadata` in the response payload. No filter params yet (Phase 3 adds those). Just makes the field visible so dashboards and downstream readers can light up.

**Files to modify/create:**
- `packages/notickets-service/src/server/lib/project-feed-reader.ts`
- `packages/notickets-service/src/server/routes/projects.ts`
- `packages/notickets-service/src/server/routes/events.ts`
- corresponding `.test.ts` files

**Expected changes:**
- Select `metadata` in every event query
- Pass through into JSON response unchanged
- Tests assert `metadata` appears in the response shape and matches what was ingested

### 6. Migrate internal callers to emit metadata
status: not_started

Make sure every internal publisher (nt-cli, MCP server publish tool, board UI publish actions, any test fixtures) emits valid `metadata.actor`. Drives the deprecation metric to zero so Phase 3's hard cutover can ship.

**Files to modify/create:**
- audit pass across `packages/notickets-service/src/client/**` for any `publish(…)` calls — add actor
- `crates/nt-mcp/src/tools/publish.rs` — wire to the same actor-resolution logic as `nt-cli`
- `scripts/seed-product-demo.sh` — pass `--agent-id seed-script --model n/a` flags (or use `nt session start` from the script)
- test fixtures across both repos that publish events — add minimal `metadata.actor` blocks

**Expected changes:**
- Every internal call passes a valid actor block (synthetic `system`-style values where appropriate)
- Metric `events_ingested_without_metadata_total` drops to ~zero in staging within one week of this task completing
- Seed script's events now show on the board with a "seed-script" actor pill

## Dependencies

- **Feature 1 (Schemas + nt session)**: must ship first. The schema definition + Rust validator + nt-cli wiring are all prerequisites for the server validation gate to mean anything.
- **`cross-platform-cli-binary` Task 5 (full MCP server port)**: MCP publish tool needs actor wiring as part of this feature's "migrate internal callers" task.
- **Decision on Option A vs B**: blocks Task 2. Default to Option A unless a prod environment with irreplaceable history exists.

## Testing Strategy

### Unit Tests

- Drizzle schema test asserts `metadata` column shape
- Ingest path test: valid metadata persisted; invalid rejected; absent metadata accepted with NULL persist
- Metrics test: counter increments only when metadata is absent; not when present

### Integration Tests

- Hono request test: POST `/v1/events` with valid metadata → 200 + DB row carries metadata; with invalid metadata → 422 + structured error; without metadata → 200 + DB row carries NULL
- Read-back test: GET `/v1/projects/:id/feed` returns events with their `metadata` populated
- Migration test: apply migration on a non-empty events table → either rows truncated (A) or backfilled (B) per chosen strategy

### Manual Testing

- Run `scripts/seed-product-demo.sh` against a staging deployment carrying Feature 1 + Feature 2 → confirm all events carry `metadata.actor`
- Verify Prometheus metric in staging shows zero `events_ingested_without_metadata_total{projectId="cge-demo"}` after seed
- Spot-check the staging dashboard's event feed includes the `metadata` field in the JSON response

## Implementation Notes

- The metric is **labelled by project and event type**, not by ip or user — keep the cardinality bounded (~ projects × event-types).
- If Option B is chosen, the `system` actor variant is a v1 schema change that must propagate through Feature 1's schemas + Rust validator before this feature can ship. Coordinate the order.
- The migration runs in a single transaction; on a large `events` table the backfill (Option B) needs batching. Document the batch size choice in the migration SQL.
- Avoid adding indexes in this phase — Phase 3 adds partial GIN indexes once the column is NOT NULL. Adding them earlier wastes write performance during the deprecation window for queries that aren't yet wired up.
- Server does **not** synthesise an actor server-side when one is absent. The hard rule is: actor comes from the client. If we synthesise we lose accountability — what we'd write is "the server saw a missing actor and made one up," which is worse than `NULL`.

## Workflow Example

```bash
# Apply the migration
pnpm --filter notickets-service drizzle:migrate

# Confirm column exists
psql "$DATABASE_URL" -c "\d events" | grep metadata

# Watch the deprecation metric
curl -s http://localhost:3000/metrics | grep events_ingested_without_metadata_total
```

## Benefits

- Storage in place for Phase 3's hard requirement — no DB work needed at cutover
- Validation runs from day one of this phase, so any malformed metadata is caught immediately rather than discovered post-cutover
- The deprecation metric is the objective gate: we ship Phase 3 when the counter is flat at zero, not when we guess callers are ready
