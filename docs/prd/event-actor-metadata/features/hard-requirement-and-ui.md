---
id: hard-requirement-and-ui
prd_id: event-actor-metadata
number: 3
title: Indexes + read APIs + UI
status: not_started
created: 2026-05-14
updated: 2026-05-21
---

# Feature: Indexes + read APIs + UI

## Description

Phase 3 of the opt-in actor-metadata rollout. Adds the indexes that make actor-keyed queries cheap, surfaces actor filters in read APIs, and renders actor pills on the board **for events that have an actor**. Unattributed events keep working unchanged — they still show on the board, just without a pill — and "show me everything Claude did this week" becomes a one-query filter that lands on an index over the opt-in subset.

There is no schema flip, no NOT NULL migration, no cutover. `metadata` stays optional on the envelope schema and the column stays nullable, forever.

By the end of this feature, every event that *has* an actor is attributable in the UI; every event that *doesn't* publishes and renders without ceremony; the actor-keyed query path is fast even on a sparse opt-in subset.

## Acceptance Criteria

- [ ] Partial GIN indexes exist on `metadata.actor.agentId` (where `actor.type='agent'`) and `metadata.actor.userId` (where `actor.type='human'`)
- [ ] Read APIs accept `?actorType=agent|human`, `?agentId=<id>`, `?userId=<id>` filters and apply them via the partial indexes
- [ ] New endpoint `/v1/projects/:projectId/actors` returns the distribution of actors that *have* posted to the project (excludes events with NULL metadata)
- [ ] Board renders an actor pill on event cards **that have an actor** — agent (icon + agentId + optional model badge if present) or human (avatar + name). Unattributed events render without a pill (no placeholder, no "unknown" label).
- [ ] Activity feed has an actor filter dropdown; selections persist via URL state. A "no actor" filter option is also supported, surfacing the unattributed subset.
- [ ] GDPR user-deletion job extends to redact `metadata.actor.userId` and `metadata.actor.email` on past events where the deleted user was the actor
- [ ] All prior schema / parity / persistence tests still pass; new tests cover filter params, the actors endpoint, the pill-present and pill-absent render paths, and the "no actor" filter

## Tasks

### 1. Add partial GIN indexes on actor identifiers
status: not_started

Partial indexes — limited to one actor variant each, and partial on `WHERE actor.type = '<variant>'` — keep the index size proportional to the opt-in subset, not the full event table. GIN over the jsonb path expression supports equality on `agentId` / `userId` directly.

**Files to modify/create:**
- `packages/notickets-service/drizzle/<timestamp>-events-actor-indexes.sql` (new)
- `packages/notickets-service/src/server/db/schema.ts` — declare indexes in drizzle metadata
- `packages/notickets-service/src/server/lib/project-feed-reader.test.ts` — `EXPLAIN` regression test

**Expected changes:**
- `CREATE INDEX events_actor_agent_id_idx ON events ((metadata->'actor'->>'agentId')) WHERE metadata->'actor'->>'type' = 'agent';`
- `CREATE INDEX events_actor_user_id_idx ON events ((metadata->'actor'->>'userId')) WHERE metadata->'actor'->>'type' = 'human';`
- EXPLAIN test confirms the agent-id-filter query uses the index (no seq scan in the plan)
- Indexes are built with `CREATE INDEX CONCURRENTLY` if/when applied to a customer-prod table; on staging the transactional migration is fine

### 2. Add `?actorType`, `?agentId`, `?userId` filters to read APIs
status: not_started

Extend the existing event-returning endpoints to accept the three filter params. Filters are validated (enum on `actorType`, non-empty string on the ids) and translated to indexed jsonb conditions. Filters combine with existing project / domain / event-type filters with AND semantics. Adding any actor filter implicitly excludes events with NULL metadata.

**Files to modify/create:**
- `packages/notickets-service/src/server/lib/project-feed-reader.ts`
- `packages/notickets-service/src/server/routes/projects.ts`
- `packages/notickets-service/src/server/routes/events.ts`
- corresponding `.test.ts` files
- `docs/api-reference.md` — document the new params

**Expected changes:**
- `?actorType=agent` → adds `metadata->'actor'->>'type' = 'agent'` condition (implicitly `metadata IS NOT NULL`)
- `?actorType=none` → adds `metadata IS NULL` condition (surface the unattributed subset)
- `?agentId=claude` → adds `metadata->'actor'->>'agentId' = 'claude'` condition (and forces `actorType=agent`)
- `?userId=<uuid>` → adds `metadata->'actor'->>'userId' = $1` condition (and forces `actorType=human`)
- Filter combinations validated server-side (`?userId` with `?actorType=agent` → 400; `?agentId` with `?actorType=none` → 400)
- Tests cover each filter alone, combinations, the `actorType=none` path, and the indexed-path EXPLAIN assertion

### 3. Add `/v1/projects/:projectId/actors` endpoint
status: not_started

Returns the distribution of actors that have published to a project (excludes events with NULL metadata — they're not "actors" in any sense the UI can render). Powers a board sidebar widget that lists "who has been pushing to this project" with counts. Aggregates over the same partial indexes the filters use.

**Files to modify/create:**
- `packages/notickets-service/src/server/lib/project-actors-reader.ts` (new)
- `packages/notickets-service/src/server/routes/projects.ts` — register route
- `packages/notickets-service/src/server/routes/projects.test.ts`

**Expected changes:**
- Endpoint returns `{ actors: [{ type, agentId?, userId?, model?, eventCount, firstSeen, lastSeen }], unattributedCount: number }` — `unattributedCount` separately surfaces the size of the NULL-metadata subset for transparency
- `model` only present in the response when present in the actor block (no synthesised `"n/a"`)
- Sorted by `eventCount` descending
- Optional `?since=<ISO8601>` to scope to a time window
- Permissioned same as `/feed` — caller must be a member of the project's team

### 4. Render actor pill on event cards (when actor present)
status: not_started

Event cards on the board show an actor pill **when `metadata.actor` is present**. Agent variants render an icon (claude / codex / generic-agent fallback) plus agentId; a model badge appears only if `model` is populated (omitted for non-LLM agents). Human variants render the user's avatar (from Kinde profile) plus display name. Cards without metadata render without a pill — no placeholder, no "unknown actor" label, no visual hint that anything is missing.

**Files to modify/create:**
- `packages/notickets-service/src/client/shared/event-card.tsx`
- `packages/notickets-service/src/client/shared/actor-pill.tsx` (new)
- `packages/notickets-service/src/client/shared/actor-icon.tsx` (new — maps agentId to icon component)
- `packages/notickets-service/src/client/shared/actor-pill.test.tsx`

**Expected changes:**
- Pill renders compactly (single line) with a tooltip showing full actor block on hover
- Model badge renders only when `actor.model` is populated; absent → no badge slot, no "n/a" text
- Agent icons covered: claude, codex, tiny-brain, github-actions, generic-agent (fallback)
- Human pill pulls avatar URL + display name from a `/v1/users/:userId/profile` lookup (cached client-side)
- Clicking the pill navigates to `/projects/:id/feed?agentId=<id>` (or `?userId=<id>`)
- Snapshot tests for: agent with model, agent without model, human, **no metadata** (asserts no pill renders)

### 5. Add actor filter to activity feeds
status: not_started

Activity-feed views (project feed, cross-project feed) get an "Actor" filter dropdown alongside existing domain / event-type filters. Selections write to URL state (`?agentId=…` etc.) so they're shareable and survive reload. The dropdown reads its options from `/v1/projects/:id/actors`, plus a "No actor" option that maps to `?actorType=none`.

**Files to modify/create:**
- `packages/notickets-service/src/client/pages/project/ui/project-feed-view.tsx`
- `packages/notickets-service/src/client/shared/actor-filter.tsx` (new)
- `packages/notickets-service/src/client/pages/project/ui/project-feed-view.test.tsx`

**Expected changes:**
- Filter dropdown shows actors from the project's history, with eventCount per option
- "No actor" option appears at the bottom with `unattributedCount` from the `/actors` endpoint
- Selection writes URL state via search params
- URL state is round-trippable — reload preserves the filter
- Keyboard-navigable (Tab + Arrow keys)

### 6. Extend GDPR user-deletion to redact actor identifiers
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

- **Feature 2 (Server validation + DB column)**: must be fully deployed. No timing gate beyond that — no "wait for the metric to drop" because there is no metric.
- **`cross-platform-cli-binary` (✅ completed 2026-05-20)**: the binary surface is stable; this feature consumes its actor-resolution outputs but doesn't depend on further binary changes.
- **Kinde profile lookup endpoint**: the human pill's avatar + display name come from `/v1/users/:userId/profile`. If that endpoint doesn't exist, add it as part of Task 4.

## Testing Strategy

### Unit Tests

- Filter parser tests: invalid combinations rejected with 400; valid ones translate to expected DB conditions; `actorType=none` path covered
- Actor pill snapshot tests: agent with model / agent without model / human / no metadata (no pill)
- Actors-endpoint shape test: `unattributedCount` reflects NULL-metadata rows separately from the `actors[]` distribution

### Integration Tests

- GET `/v1/projects/:id/feed?agentId=claude` → returns only Claude-authored events; EXPLAIN confirms index use
- GET `/v1/projects/:id/feed?actorType=none` → returns only events with `metadata IS NULL`
- GET `/v1/projects/:id/actors` → returns sorted actor distribution + correct `unattributedCount`
- POST `/v1/events` without `metadata` → still succeeds with 200 (this feature does **not** change ingest behaviour)
- User-deletion E2E: create user, publish events, delete user via webhook, confirm events redacted

### Manual Testing

- Open the staging board; confirm event cards with metadata show pills and cards without don't
- Click an agent pill → navigate to the filtered feed; confirm only that agent's events appear
- Use the actor filter dropdown including the "No actor" option; confirm URL state updates and reload preserves the filter

## Implementation Notes

- Partial GIN indexes are small (only matching rows) and naturally scale with the opt-in subset. On staging this is fine; on customer-prod, build with `CREATE INDEX CONCURRENTLY` and apply as a manual operations step rather than a transactional migration.
- The actor pill needs a fallback for unknown agentIds — don't 404 the UI when a new agent shows up. The `actor-icon.tsx` resolver returns a generic icon for any agentId it doesn't recognise.
- The pill must never render a placeholder for missing actors. "No pill" is the correct render for unattributed events. Resist the temptation to show a "unknown actor" / "anonymous" label — it visually penalises a permanent valid state and pressures callers toward attribution they didn't choose.
- GDPR redaction is point-in-time only. A complete "right to be forgotten" implementation would key-shred per-user data; that's a much larger initiative tracked separately.
- The actors endpoint's aggregation uses `GROUP BY metadata->'actor'->>'agentId'` (or `userId`) with `WHERE metadata IS NOT NULL`. The partial indexes support both the grouping and the WHERE. Verify with EXPLAIN.

## Workflow Example

```bash
# Apply Phase 3 migrations (indexes only — no schema flip)
pnpm --filter notickets-service drizzle:migrate

# Query an agent's activity
curl "https://api-staging.no-tickets.com/v1/projects/cge-demo/feed?agentId=claude" \
  -H "Authorization: Bearer ..."

# See the unattributed subset
curl "https://api-staging.no-tickets.com/v1/projects/cge-demo/feed?actorType=none" \
  -H "Authorization: Bearer ..."

# Get the actor distribution
curl "https://api-staging.no-tickets.com/v1/projects/cge-demo/actors" \
  -H "Authorization: Bearer ..."
# → { actors: [{type:'agent',agentId:'claude',eventCount:1234,...}, ...],
#     unattributedCount: 567 }
```

## Benefits

- Cheap actor-keyed queries — partial GIN indexes make "all activity by X" O(matches), not O(events), even on a sparse opt-in subset
- UI surfaces the answer to "who did this?" on cards that have an actor, with zero visual ceremony on cards that don't
- The opt-in subset becomes a first-class queryable surface (filters + actors endpoint + UI dropdown) without forcing the rest of the event log through it
- GDPR redaction story is closed for the human-actor subset
