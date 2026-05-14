---
id: hard-requirement-and-ui
prd_id: event-actor-metadata
number: 3
title: Hard requirement + indexes + read APIs + UI
status: not_started
created: 2026-05-14
updated: 2026-05-14
---

# Feature: Hard requirement + indexes + read APIs + UI

## Description

Phase 3 of the actor-metadata rollout. Flips the server from "metadata tolerated" to "metadata required." Adds the indexes that make actor-keyed queries cheap. Surfaces actor filters in read APIs and renders actor pills on the board.

This is the cutover phase. After this ships, `nt publish` without a resolvable actor errors at the server with `validation_error`. The Phase 2 deprecation metric must be flat at zero for at least a week before this feature ships — that's the objective signal that internal callers have migrated.

By the end of this feature, every event in the system is attributable, every event card on the board shows who produced it, and "show me everything Claude did this week" is a one-query filter that lands on an index.

## Acceptance Criteria

- [ ] `eventEnvelopeSchema` requires `metadata` (no longer optional)
- [ ] `events.metadata` column is NOT NULL; migration sets the constraint atomically
- [ ] Partial GIN indexes exist on `metadata.actor.agentId` (where `actor.type='agent'`) and `metadata.actor.userId` (where `actor.type='human'`)
- [ ] Server rejects actor-less events with 422 + structured error: `{ error: "validation_error", path: "metadata.actor", message: "required" }`
- [ ] Read APIs accept `?actorType=agent|human`, `?agentId=<id>`, `?userId=<id>` filters and apply them via the partial indexes
- [ ] New endpoint `/v1/projects/:projectId/actors` returns the distribution of actors in the project (agent + human breakdown, count per actor)
- [ ] Board renders an actor pill on every event card — agent (icon + agentId + model badge) or human (avatar + name)
- [ ] Activity feed has an actor filter dropdown; selections persist via URL state
- [ ] GDPR user-deletion job extends to redact `metadata.actor.userId` and `metadata.actor.email` on past events
- [ ] All prior actor-coverage / parity tests still pass; new tests cover the strict-required path, the filter params, and the actors endpoint

## Tasks

### 1. Flip `metadata` from optional to required on the server
status: not_started

`eventEnvelopeSchema.metadata` becomes required. The ingest path now relies on the schema gate to reject actor-less events with a structured validation error. The deprecation metric from Feature 2 is removed since absent metadata is no longer a tolerated state.

**Files to modify/create:**
- `packages/notickets-service/src/server/events/publish-batch.ts`
- `packages/notickets-service/src/server/events/ingest.ts`
- `packages/notickets-service/src/server/observability/metrics.ts` — remove deprecation counter
- `packages/notickets-service/src/server/events/publish-batch.test.ts`
- `packages/notickets-service/src/server/events/ingest.test.ts`

**Expected changes:**
- `metadata: eventMetadataSchema` (no `.optional()`)
- Test cases flip: previously "absent metadata → 200" now becomes "absent metadata → 422 with validation_error naming `metadata.actor`"
- Deprecation counter + its dashboard panel removed

### 2. Make `events.metadata` NOT NULL
status: not_started

Drizzle migration adding the NOT NULL constraint. Pre-flight check confirms zero NULL rows; if any exist (shouldn't, post-deprecation-window) the migration aborts. Reversibility note: dropping NOT NULL is trivial; restoring the column post-drop is not, so we don't drop it.

**Files to modify/create:**
- `packages/notickets-service/src/server/db/schema.ts`
- `packages/notickets-service/drizzle/<timestamp>-events-metadata-not-null.sql` (new)
- `packages/notickets-service/src/server/db/schema.test.ts`

**Expected changes:**
- Drizzle column metadata changed to `.notNull()`
- Migration runs `ALTER TABLE events ALTER COLUMN metadata SET NOT NULL` after asserting zero nulls
- Schema test asserts the constraint via information_schema lookup

### 3. Add partial GIN indexes on actor identifiers
status: not_started

Partial indexes — limited to one actor variant each — keep the index small and make the "all activity by X" query land in one lookup. GIN over the jsonb path expression supports equality on `agentId` / `userId` directly.

**Files to modify/create:**
- `packages/notickets-service/drizzle/<timestamp>-events-actor-indexes.sql` (new)
- `packages/notickets-service/src/server/db/schema.ts` — declare indexes in drizzle metadata
- `packages/notickets-service/src/server/lib/project-feed-reader.test.ts` — `EXPLAIN` regression test

**Expected changes:**
- `CREATE INDEX events_actor_agent_id_idx ON events ((metadata->'actor'->>'agentId')) WHERE metadata->'actor'->>'type' = 'agent';`
- `CREATE INDEX events_actor_user_id_idx ON events ((metadata->'actor'->>'userId')) WHERE metadata->'actor'->>'type' = 'human';`
- EXPLAIN test confirms the agent-id-filter query uses the index (no seq scan in the plan)

### 4. Add `?actorType`, `?agentId`, `?userId` filters to read APIs
status: not_started

Extend the existing event-returning endpoints to accept the three filter params. Filters are validated (enum on `actorType`, non-empty string on the ids) and translated to indexed jsonb conditions. Filters combine with existing project / domain / event-type filters with AND semantics.

**Files to modify/create:**
- `packages/notickets-service/src/server/lib/project-feed-reader.ts`
- `packages/notickets-service/src/server/routes/projects.ts`
- `packages/notickets-service/src/server/routes/events.ts`
- corresponding `.test.ts` files
- `docs/api-reference.md` — document the new params

**Expected changes:**
- `?actorType=agent` → adds `metadata->'actor'->>'type' = 'agent'` condition
- `?agentId=claude` → adds `metadata->'actor'->>'agentId' = 'claude'` condition (and forces `actorType=agent`)
- `?userId=<uuid>` → adds `metadata->'actor'->>'userId' = $1` condition (and forces `actorType=human`)
- Filter combinations validated server-side (`?userId` with `?actorType=agent` → 400)
- Tests cover each filter alone, combinations, and the indexed-path EXPLAIN assertion

### 5. Add `/v1/projects/:projectId/actors` endpoint
status: not_started

Returns the distribution of actors that have published to a project. Powers a board sidebar widget that lists "who has been pushing to this project" with counts. Aggregates over the same partial indexes the filters use.

**Files to modify/create:**
- `packages/notickets-service/src/server/lib/project-actors-reader.ts` (new)
- `packages/notickets-service/src/server/routes/projects.ts` — register route
- `packages/notickets-service/src/server/routes/projects.test.ts`

**Expected changes:**
- Endpoint returns `{ actors: [{ type, agentId?, userId?, model?, eventCount, firstSeen, lastSeen }] }`
- Sorted by `eventCount` descending
- Optional `?since=<ISO8601>` to scope to a time window
- Permissioned same as `/feed` — caller must be a member of the project's team

### 6. Render actor pill on event cards
status: not_started

Every event card on the board shows an actor pill. Agent variants render an icon (claude / codex / generic-agent fallback) plus agentId and a model badge. Human variants render the user's avatar (from Kinde profile) plus display name. Click-through opens an actor-filtered feed.

**Files to modify/create:**
- `packages/notickets-service/src/client/shared/event-card.tsx`
- `packages/notickets-service/src/client/shared/actor-pill.tsx` (new)
- `packages/notickets-service/src/client/shared/actor-icon.tsx` (new — maps agentId to icon component)
- `packages/notickets-service/src/client/shared/actor-pill.test.tsx`

**Expected changes:**
- Pill renders compactly (single line) with a tooltip showing full actor block on hover
- Agent icons covered: claude, codex, tiny-brain, github-actions, generic-agent (fallback)
- Human pill pulls avatar URL + display name from a `/v1/users/:userId/profile` lookup (cached client-side)
- Clicking the pill navigates to `/projects/:id/feed?agentId=<id>` (or `?userId=<id>`)
- Snapshot tests for the major variants

### 7. Add actor filter to activity feeds
status: not_started

Activity-feed views (project feed, cross-project feed) get an "Actor" filter dropdown alongside existing domain / event-type filters. Selections write to URL state (`?agentId=…` etc.) so they're shareable and survive reload. The dropdown reads its options from `/v1/projects/:id/actors`.

**Files to modify/create:**
- `packages/notickets-service/src/client/pages/project/ui/project-feed-view.tsx`
- `packages/notickets-service/src/client/shared/actor-filter.tsx` (new)
- `packages/notickets-service/src/client/pages/project/ui/project-feed-view.test.tsx`

**Expected changes:**
- Filter dropdown shows actors from the project's history, with eventCount per option
- Selection writes URL state via search params
- URL state is round-trippable — reload preserves the filter
- Keyboard-navigable (Tab + Arrow keys)

### 8. Extend GDPR user-deletion to redact actor identifiers
status: not_started

The existing user-deletion job (triggered on Kinde delete-user webhook) walks the user's owned data. Extend it to issue a single UPDATE that redacts `metadata.actor.userId` and `metadata.actor.email` on every event where that user is the actor. Replaces with `{ type: 'human', userId: null, email: null }` — the event itself stays, the actor becomes anonymous-historical.

**Files to modify/create:**
- `packages/notickets-service/src/server/jobs/user-deletion.ts`
- `packages/notickets-service/src/server/jobs/user-deletion.test.ts`
- `docs/operations/gdpr.md`

**Expected changes:**
- UPDATE: `SET metadata = jsonb_set(metadata, '{actor}', '{"type":"human","userId":null,"email":null}'::jsonb)` where `metadata->'actor'->>'userId' = $1`
- Test: seed events for a user; trigger deletion; assert no more rows match the userId; assert events still exist
- Runbook note: redaction is point-in-time, not cryptographic; documented as a known limitation in the PRD

## Dependencies

- **Feature 2 (Server validation + DB column)**: must be fully deployed and the deprecation metric flat at zero for ≥7 days before this feature ships. Hard gate.
- **`cross-platform-cli-binary` Task 4 + 5**: every internal CLI / MCP publisher must already emit metadata before the server starts rejecting actor-less events.
- **Kinde profile lookup endpoint**: the human pill's avatar + display name come from `/v1/users/:userId/profile`. If that endpoint doesn't exist, add it as part of Task 6.

## Testing Strategy

### Unit Tests

- Schema test: `eventEnvelopeSchema.parse({ no metadata })` throws with the documented error shape
- Filter parser tests: invalid combinations rejected with 400; valid ones translate to expected DB conditions
- Actor pill snapshot tests: every variant renders correctly with realistic data
- Migration test: NOT NULL migration aborts on NULL rows (defensive)

### Integration Tests

- POST `/v1/events` without `metadata` → 422 with `validation_error` naming `metadata.actor`
- GET `/v1/projects/:id/feed?agentId=claude` → returns only Claude-authored events; EXPLAIN confirms index use
- GET `/v1/projects/:id/actors` → returns sorted actor distribution
- User-deletion E2E: create user, publish events, delete user via webhook, confirm events redacted

### Manual Testing

- Open the staging board; confirm every event card shows an actor pill
- Click an agent pill → navigate to the filtered feed; confirm only that agent's events appear
- Use the actor filter dropdown; confirm URL state updates; reload; confirm filter persists

## Implementation Notes

- The NOT NULL migration is the irreversible step in this phase. Pre-flight check (`SELECT count(*) FROM events WHERE metadata IS NULL`) must return zero before applying. The migration script aborts with a clear error if not.
- Partial GIN indexes are small (only matching rows) but still take real time to build on a large table. On staging this is fine; if a customer-prod table exists, build with `CREATE INDEX CONCURRENTLY` and add it as a manual operations step rather than a transactional migration.
- The actor pill needs a fallback for unknown agentIds — don't 404 the UI when a new agent shows up. The `actor-icon.tsx` resolver returns a generic icon for any agentId it doesn't recognise.
- GDPR redaction is point-in-time only. A complete "right to be forgotten" implementation would key-shred per-user data; that's a much larger initiative tracked separately.
- The actors endpoint's aggregation uses `GROUP BY metadata->'actor'->>'agentId'` (or `userId`). Without the partial index this is a full scan; *with* it, the partial-GIN supports the grouping. Verify with EXPLAIN.

## Workflow Example

```bash
# Pre-flight check before cutover
psql "$DATABASE_URL" -c "SELECT count(*) FROM events WHERE metadata IS NULL"
# → 0

# Apply Phase 3 migrations
pnpm --filter notickets-service drizzle:migrate

# Verify the gate
curl -X POST https://api-staging.no-tickets.com/v1/events \
  -H "Authorization: Bearer $NO_TICKETS_TOKEN" \
  -d '[{"type":"ai.completion.recorded.v1","data":{...},"source":{...}}]'
# → 422 { error: "validation_error", path: "metadata.actor", message: "required" }

# Query an agent's activity
curl "https://api-staging.no-tickets.com/v1/projects/cge-demo/feed?agentId=claude" \
  -H "Authorization: Bearer ..."
```

## Benefits

- Full actor accountability — every event in the system names its producer
- Cheap actor-keyed queries — partial GIN indexes make "all activity by X" O(matches), not O(events)
- UI surfaces the answer to "who did this?" on every card without a click
- The Phase 1/2 deprecation gate is finally closed — the security argument behind the broader push-token grant is now load-bearing on a real schema constraint, not a polite request
