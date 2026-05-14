---
id: event-actor-metadata
title: Actor metadata on every event + `nt session` lifecycle
version: 1.0.0
status: not_started
created: 2026-05-14
updated: 2026-05-14
author: Andy Richardson
---

# Actor metadata on every event + `nt session` lifecycle

## Purpose and Goals

Every event published to no-tickets must carry a typed `metadata.actor` block that names the producer. The `nt` binary owns actor resolution: agents call `nt session start` once at boot to declare themselves, humans rely on credentials from `nt init`, and `nt publish` stamps the resolved actor onto every envelope automatically. The actor block lives outside `data` so domain reducers stay pure, validated against one canonical schema regardless of event type.

This PRD also closes the loop on a structural issue surfaced while seeding the cge-demo product board: until very recently, the only way to publish non-`ai.*` events was a logged-in browser session, because push tokens were hardcoded to `ai.write` only. We broadened push tokens to write every reserved domain as a tactical fix (commit landed 2026-05-13); this PRD adds the accountability layer that makes that broadening durable rather than tactical. Every event names its actor, so a leaked or misused token leaves an auditable trail rather than an anonymous one.

Goals:

- Every event in the database has a non-null `metadata.actor` block.
- One canonical actor schema. `ai.*` event payloads stop carrying duplicated agent identity fields.
- `nt` binary is the integration point for actor resolution. Callers do not sprinkle env vars or repeat agent identity on every publish.
- Push tokens authenticate the project; the actor block identifies the producer. Two independent concerns.
- Schema and validator parity between TS (`packages/schemas`) and Rust (`nt-schemas` crate) — same definition, two binding outputs.
- No prompt or completion content in `metadata`. Identity + cost only.

## User Needs

### Target Audience

- **Agent harnesses** (Claude Code, Codex, tiny-brain) integrating `nt` to publish events on behalf of an LLM session
- **Human operators** publishing events from CLI / web tools
- **Product / engineering reviewers** querying "who did what" across the event log
- **Compliance / audit** needing accountability for every state change
- **Future per-language wrappers** (TS / Python / Go) inheriting the actor model for free by spawning `nt`

### User Stories

1. As an agent harness, I want to declare my identity once per session via `nt session start` so I don't repeat agent / model / provider fields on every publish.
2. As an LLM-driven publisher, I want my model and thinking-effort recorded on every event I produce so reviewers can attribute behaviour without manual instrumentation.
3. As a human publishing from `nt`, I want my actor identity (userId + email) attached automatically from my session credentials with zero extra flags.
4. As a project reviewer, I want to query "every event Claude produced in the last week" across all domains in one filter, regardless of event type.
5. As a security operator, I want every event to name a real actor so a leaked push token surfaces in the audit trail rather than blending into anonymous traffic.
6. As an `nt` contributor, I want a clear precedence chain (flags → session file → credentials) and a structured-error exit when no actor can be resolved.
7. As a per-language wrapper author, I want to spawn `nt` and inherit the actor model for free, without reimplementing the resolution logic in TS / Python / Go.

## Features and Functionality

This PRD is delivered in four phases. Each phase is a feature.

### Feature 1: Schemas + `nt session` + `nt publish` actor wiring
**File**: [features/schemas-and-nt-session.md](features/schemas-and-nt-session.md)
**Status**: not_started
**Description**: Define `actorSchema` / `eventMetadataSchema` in `packages/schemas`; ship Rust validator parity in `nt-schemas`; implement `nt session start / show / end`; wire actor resolution into `nt publish`. Server-side schema accepts `metadata` as optional in this phase so the change is non-breaking.

### Feature 2: Server-side validation gate + DB column + backfill
**File**: [features/server-validation-and-storage.md](features/server-validation-and-storage.md)
**Status**: not_started
**Description**: Add `metadata` jsonb column to `events`. Decide and apply migration strategy (drop existing rows per no-v1-backcompat policy, or backfill with `system` placeholder). Server validates `metadata` against the canonical schema on ingest. One-release deprecation window where actor-less events log a metric but are accepted.

### Feature 3: Hard requirement + indexes + read APIs + UI
**File**: [features/hard-requirement-and-ui.md](features/hard-requirement-and-ui.md)
**Status**: not_started
**Description**: Flip `metadata` from optional to required in `eventEnvelopeSchema`. Make `events.metadata` NOT NULL. Add partial GIN indexes on actor identifiers. Surface `metadata` on read APIs with filter params. Render actor pills on the board; activity feeds become filterable by actor.

### Feature 4: Per-language wrappers inherit `nt session`
**File**: [features/per-language-wrappers.md](features/per-language-wrappers.md)
**Status**: not_started
**Description**: TS / Python / Go wrappers call `nt session start` at init and `nt session end` on shutdown. Each wrapper exposes a `withActor()` helper for one-off overrides. Conformance tests per wrapper assert the actor block flows correctly. Pairs with Phase 4 of the `cross-platform-cli-binary` fix.

## Design and User Experience

### Envelope shape (v1)

```json
{
  "type":   "product.feature.status_changed.v1",
  "data":   { "featureId": "feat-1", "fromStatus": "review", "toStatus": "done" },
  "metadata": {
    "actor": {
      "type": "agent",
      "agentId": "claude",
      "model": "claude-opus-4-7",
      "provider": "anthropic",
      "thinkingEffort": "high",
      "sessionId": "sess-abc123",
      "callId": "call-xyz",
      "promptTokens": 1234,
      "completionTokens": 567,
      "latencyMs": 812
    }
  },
  "source": { "name": "cli", "sdkVersion": "0.1.0" },
  "traceId": "...",
  "occurredAt": "..."
}
```

`metadata` is a top-level envelope field, sibling to `data` and `source`. Reasoning:

- **`data` stays domain-pure.** A product reducer reads `{ featureId, fromStatus, toStatus }` and nothing else.
- **One schema gate, one query path.** `metadata.actor.agentId = 'claude'` is one jsonb path expression, regardless of event domain.
- **`metadata` is a namespace, not a single field.** Future additions (`metadata.trace` for full OTel, `metadata.causation` for upstream-event-explainability) live here without colliding with `data`.

### Actor schema (v1)

```ts
const agentActorSchema = z.object({
  type: z.literal('agent'),
  // Mandatory — minimum identity.
  agentId: z.string().min(1),       // 'claude', 'codex', 'tiny-brain', 'github-actions'
  model: z.string().min(1),         // 'claude-opus-4-7', 'gpt-5', 'n/a' for non-LLM systems

  // Optional — per-call / per-session enrichment.
  provider: z.string().min(1).optional(),
  sessionId: z.string().min(1).optional(),
  callId: z.string().min(1).optional(),
  thinkingEffort: z.enum(['low', 'medium', 'high']).optional(),
  promptTokens: z.number().int().nonnegative().optional(),
  completionTokens: z.number().int().nonnegative().optional(),
  latencyMs: z.number().int().nonnegative().optional(),
}).strict();

const humanActorSchema = z.object({
  type: z.literal('human'),
  userId: z.string().min(1),
  email: z.string().email().optional(),
}).strict();

const actorSchema = z.discriminatedUnion('type', [agentActorSchema, humanActorSchema]);
const eventMetadataSchema = z.object({ actor: actorSchema }).strict();
```

What does NOT belong (non-negotiable for v1):

- No prompt content
- No completion text
- No tool-call arguments
- No reasoning traces

Those live in `ai.*` event `data` when they are themselves the fact being recorded. `metadata.actor` is identity + cost only.

### `nt session` lifecycle

```
nt session start \
  --agent claude \
  --model claude-opus-4-7 \
  --provider anthropic \
  --thinking-effort high \
  --session-id sess-abc123
# → writes ~/.notickets/active-session.json (atomic temp+rename)

nt publish --type product.feature.status_changed.v1 --data '{...}'
# → reads active session, stamps metadata.actor automatically

nt session show     # prints the active session, flags expiry
nt session end      # deletes active-session.json (idempotent)
```

Default session lifetime: 24 hours from `startedAt`. Configurable via `--max-age-hours N` (hard cap 7 days). After expiry, `nt publish` falls back to credential-based human actor or errors.

Multi-session concurrency: `NT_SESSION_FILE=/path/to/another.json` points at an alternate file. The env var carries an opaque path only — not LLM details.

### `nt publish` actor resolution precedence

```
1. --actor-* flags present                            → flags win
2. NT_SESSION_FILE env var set                        → read that file
3. ~/.notickets/active-session.json present + fresh   → use it
4. session credentials present (nt init was run)      → human actor from creds
5. otherwise                                          → exit 5 (not_authenticated):
   "actor not resolved. Run `nt session start` or pass --actor-* flags."
```

### Directory Structure

```
~/.notickets/
├── credentials              # existing — human session token from `nt init`
├── config.json              # existing — project → push-token registry
└── active-session.json      # NEW — atomic file written by `nt session start`
```

## Release Criteria

### Functional Requirements

- [ ] `actorSchema`, `eventMetadataSchema`, and types are exported from `@magic-ingredients/no-tickets-schemas`
- [ ] Rust `nt-schemas` crate provides a `validate_metadata()` function with TS-parity issue shapes
- [ ] `nt session start / show / end` subcommands implemented; file is atomically written and stale-detectable
- [ ] `nt publish` resolves actor per the precedence chain; emits `metadata.actor` on every envelope
- [ ] Server-side `eventEnvelopeSchema` accepts and (in Phase 3) requires `metadata`
- [ ] `events.metadata` jsonb column persists actor on every row; NOT NULL in Phase 3
- [ ] Partial GIN indexes on `metadata.actor.agentId` and `metadata.actor.userId`
- [ ] Read APIs surface `metadata` and accept `?actorType`, `?agentId`, `?userId` filters
- [ ] New endpoint `/v1/projects/:projectId/actors` returns the distribution of actors in the project
- [ ] Board renders an actor pill on every event card
- [ ] Activity feed is filterable by actor
- [ ] Existing GDPR user-deletion job extends to redact `metadata.actor.userId` / `email`
- [ ] `ai.*` event-type `data` schemas no longer duplicate `agentId` / `model` / `provider` (those move to metadata)
- [ ] Per-language wrappers (TS / Python / Go) spawn `nt session start` at init and `nt session end` on shutdown

### Usability Requirements

- [ ] Zero-config happy path for human pushes: after `nt init`, `nt publish` works with no actor flags
- [ ] Zero-extra-config happy path for agents: after `nt session start --agent X --model Y`, every `nt publish` stamps the actor automatically
- [ ] `nt session show` reports the active session in JSON, including expiry warning when stale
- [ ] `nt publish` with no actor resolvable returns structured error JSON (exit 5) naming the resolution path
- [ ] Actor pill renders without truncation on standard board card widths
- [ ] Activity-feed actor filter is keyboard-navigable

### Technical Requirements

- [ ] TS↔Rust validator parity test in `nt-schemas` covers all actor variants (human, agent), all required-field violations, type-discrimination
- [ ] `active-session.json` write is atomic (temp + rename)
- [ ] `nt session` subcommands are covered by `assert_cmd`-driven integration tests
- [ ] `nt publish` resolution precedence covered by table-driven tests (flags > NT_SESSION_FILE > active file > credentials > error)
- [ ] Server-side metric `events_ingested_without_metadata_total` exists during Phase 2 deprecation window
- [ ] Migration is reversible within Phase 2 (DROP COLUMN remains safe until Phase 3 cutover)
- [ ] Stryker mutation review clean on `nt session` and actor-resolution paths
- [ ] Schemas-bundle GH Release artefact (sister fix Task 6/7) includes `eventMetadataSchema` as a top-level entry

## Success Metrics (KPIs)

- **Actor coverage**: % of events in `events` table with non-null `metadata.actor`. Target: 100% post-Phase 3.
- **Resolution-error rate**: count of `not_authenticated:actor-not-resolved` errors per week from `nt publish`. Target: < 5 (excluding intentional test invocations) once internal callers migrate.
- **Agent attribution depth**: % of `agent` actors that include `model` AND `sessionId`. Target: > 90% within a month of Phase 3.
- **`ai.*` payload size reduction**: payload byte-size shrink for `ai.completion.recorded.v1` and `ai.task.completed.v1` after duplicated identity fields move out of `data`. Target: 15–25% smaller.
- **Cross-domain agent query latency**: P95 of `SELECT … FROM events WHERE metadata->'actor'->>'agentId' = $1 AND project_id = $2` on a 1M-row table. Target: < 50 ms with the partial index.

## Constraints and Dependencies

### Technical Constraints

- Push-token-authenticated callers without a session and without flags MUST error at Phase 3 cutover. This is a deliberate breaking change.
- `metadata` lives in jsonb, not normalised columns. Reducer queries use jsonb path expressions, not joins.
- Stale-session detection is timestamp-based, not pid-liveness-based. PID is recorded but not currently checked (cross-platform pid validation adds complexity for marginal benefit).
- TS↔Rust validator parity is the contract; if shapes diverge, parity tests fail CI.

### Dependencies

- **`cross-platform-cli-binary` fix (in flight)**: Task 4 (full CLI port) absorbs the `nt session` subcommand; Task 4a (structured-error contract) provides the `not_authenticated` exit code; Task 4b (`--stream` mode) inherits actor-resolution-at-startup; Task 5 (MCP server) requires actor wiring on its publish tool too.
- **Sister fix `client-roadmap-server-prerequisites`**: the JSON Schema bundle in GH Releases needs to include `eventMetadataSchema`. One-line addition to `build-json-schema-bundle.ts` when this PRD lands.
- **Permissions-broadening commit (2026-05-13)**: this PRD is the accountability counterweight to that change. Both must be in place by Phase 3 cutover for the security argument to hold.

### Known Limitations

- **No `system` actor variant in v1.** CI bots, cron jobs, and migrations get modelled as `agent` with `agentId: 'github-actions'` / `'cron'` / `'migration'` and `model: 'n/a'`. If this proves ugly in practice we add `system` as a v1.1 variant; the discriminated union accepts new branches cleanly.
- **No top-level `metadata.version` field.** Discriminated unions handle additions; a breaking shape change would version per `actor.type` (e.g. `agent.v2`) the same way event types are versioned.
- **No remote validation on `nt session start`** in v1. The actor block validates locally; if the server rejects it at first publish, that's how the harness learns about a typo.
- **GDPR redaction is point-in-time, not cryptographic.** A deleted user's past events are stamped with `userId: null` but the event itself remains. Cryptographic erasure (key-shred per-user) is a separate concern.
- **No per-token actor restriction.** A push token authorised for project X can still emit any `actor.agentId`. Trust is enforced at the project / token boundary, not the actor identity boundary. The actor block is for accountability and observability, not access control.
