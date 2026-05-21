---
id: event-actor-metadata
title: Actor metadata on every event + `no-tickets session` lifecycle
version: 1.0.0
status: not_started
created: 2026-05-14
updated: 2026-05-14
author: Andy Richardson
---

# Actor metadata on every event + `no-tickets session` lifecycle

## Purpose and Goals

Events published to no-tickets *may* carry a typed `metadata.actor` block that names the producer. Actor attribution is **opt-in**: callers who want their events attributed run `no-tickets session start` once at boot to declare themselves, and every subsequent `no-tickets publish` in that environment stamps the actor block automatically. Callers who don't opt in publish unattributed events — a permanently valid state. The `no-tickets` binary owns actor resolution; the actor block lives outside `data` so domain reducers stay pure, validated against one canonical schema regardless of event type.

This PRD also closes the loop on a structural issue surfaced while seeding the cge-demo product board: until very recently, the only way to publish non-`ai.*` events was a logged-in browser session, because push tokens were hardcoded to `ai.write` only. We broadened push tokens to write every reserved domain as a tactical fix (commit landed 2026-05-13); this PRD adds the accountability *option* that makes that broadening defensible. Callers who want an audit trail get one — declaratively, on their own terms — without forcing every push surface through a server-mediated identity flow.

Goals:

- Events published *during an active session* carry a `metadata.actor` block. Events published outside a session are unattributed (`metadata` absent) and that is permanently valid.
- One canonical actor schema. `ai.*` event payloads stop carrying duplicated agent identity fields.
- `no-tickets` binary is the integration point for actor resolution. Callers do not sprinkle env vars or repeat agent identity on every publish.
- Push tokens authenticate the project; the actor block — when present — identifies the producer. Two independent concerns; neither is load-bearing on the other.
- Schema and validator parity between TS (`packages/schemas`) and Rust (`nt-schemas` crate) — same definition, two binding outputs.
- No prompt or completion content in `metadata`. Identity + cost only.
- **No environment-sniffing.** The CLI never infers actor identity from env vars like `CLAUDECODE`, `GITHUB_ACTIONS`, or `CI`. Attribution is what the caller explicitly declared, never what the environment implies.

## User Needs

### Target Audience

- **Agent harnesses** (Claude Code, Codex, tiny-brain) integrating `no-tickets` to publish events on behalf of an LLM session
- **Human operators** publishing events from CLI / web tools
- **Product / engineering reviewers** querying "who did what" across the event log
- **Compliance / audit** wanting an *optional* audit trail for state changes
- **Future per-language wrappers** (TS / Python / Go) inheriting the actor model for free by spawning `no-tickets`

### User Stories

1. As an agent harness, I want to declare my identity once per session via `no-tickets session start` so I don't repeat agent identity fields on every publish.
2. As an LLM-driven publisher, I want my model and thinking-effort recorded on events I produce *during a declared session* so reviewers can attribute behaviour without manual instrumentation.
3. As a human publishing from `no-tickets`, I want my actor identity (userId + email) attached automatically from my session credentials with zero extra flags.
4. As a project reviewer, I want to query "every event Claude produced in the last week" across all domains in one filter, regardless of event type — accepting that events from unattributed publishers won't appear.
5. As a caller who hasn't declared a session, I want my first publish to print a one-time hint telling me how to opt into actor attribution, and then never bother me again.
6. As a per-language wrapper author, I want to spawn `no-tickets` and inherit the actor model for free, without reimplementing the resolution logic in TS / Python / Go.

## Features and Functionality

This PRD is delivered in four phases. Each phase is a feature.

### Feature 1: Schemas + `no-tickets session` + `no-tickets publish` actor wiring
**File**: [features/schemas-and-nt-session.md](features/schemas-and-nt-session.md)
**Status**: not_started
**Description**: Define `actorSchema` / `eventMetadataSchema` in `packages/schemas`; ship Rust validator parity in `nt-schemas`; implement `no-tickets session start / show / end`; wire actor resolution into `no-tickets publish`; add the first-publish hint mechanic. Server-side schema accepts `metadata` as optional (and stays that way — there is no later phase that flips this).

### Feature 2: Server-side validation gate + DB column
**File**: [features/server-validation-and-storage.md](features/server-validation-and-storage.md)
**Status**: not_started
**Description**: Add `metadata` jsonb column to `events` (nullable, permanently). Server validates `metadata` against the canonical schema on ingest **when present**; absent metadata is accepted and stored as `NULL`. No deprecation metric, no migration of existing rows (they stay NULL). Internal callers gain the option to declare actors; none are forced.

### Feature 3: Indexes + read APIs + UI
**File**: [features/hard-requirement-and-ui.md](features/hard-requirement-and-ui.md)
**Status**: not_started
**Description**: Add partial GIN indexes on actor identifiers (on the rows that have them). Surface `metadata` on read APIs with filter params. Render actor pills on event cards *when actor is present*; activity feeds become filterable by actor; unattributed events still show but without a pill. No envelope-schema flip, no NOT NULL constraint.

### Feature 4: Per-language wrappers inherit `no-tickets session`
**File**: [features/per-language-wrappers.md](features/per-language-wrappers.md)
**Status**: not_started
**Description**: TS / Python / Go wrappers *may* call `no-tickets session start` at init and `no-tickets session end` on shutdown — driven by whether the caller passed actor config to the wrapper constructor. Each wrapper exposes a `withActor()` helper for one-off overrides. Conformance tests per wrapper assert that when actor config is supplied, the actor block flows through; when omitted, events publish unattributed. Pairs with Phase 4 of the `cross-platform-cli-binary` fix.

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

`metadata` is a top-level envelope field, sibling to `data` and `source`. It is **permanently optional** on the wire and in the database. Reasoning:

- **`data` stays domain-pure.** A product reducer reads `{ featureId, fromStatus, toStatus }` and nothing else.
- **One schema gate, one query path.** `metadata.actor.agentId = 'claude'` is one jsonb path expression, regardless of event domain.
- **`metadata` is a namespace, not a single field.** Future additions (`metadata.trace` for full OTel, `metadata.causation` for upstream-event-explainability) live here without colliding with `data`.

### Actor schema (v1)

```ts
const agentActorSchema = z.object({
  type: z.literal('agent'),
  // Mandatory — minimum identity is just agentId.
  agentId: z.string().min(1),       // 'claude', 'codex', 'tiny-brain', 'github-actions'

  // Optional — per-session / per-call enrichment.
  model: z.string().min(1).optional(),       // 'claude-opus-4-7', 'gpt-5'; omit for non-LLM systems
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

Rationale for the minimum-identity cut: `agentId` is the only field that's useful in isolation. `model` is rich-but-optional — for LLM agents it should be filled, for CI bots / migrations / cron there's no meaningful model and the field is simply omitted rather than padded with sentinel strings like `"n/a"`. No `"n/a"` values anywhere in this design.

What does NOT belong (non-negotiable for v1):

- No prompt content
- No completion text
- No tool-call arguments
- No reasoning traces

Those live in `ai.*` event `data` when they are themselves the fact being recorded. `metadata.actor` is identity + cost only.

### `no-tickets session` lifecycle

```
no-tickets session start \
  --agent claude \
  --model claude-opus-4-7 \
  --provider anthropic \
  --thinking-effort high \
  --session-id sess-abc123
# → writes <config-dir>/active-session.json (atomic temp+rename)

no-tickets publish --type product.feature.status_changed.v1 --data '{...}'
# → reads active session, stamps metadata.actor automatically

no-tickets session show     # prints the active session, flags expiry
no-tickets session end      # deletes active-session.json + clears the hint marker (idempotent)
```

`<config-dir>` resolves via `paths::config_dir()` per ADR-0002 (`directories::ProjectDirs::from("com", "magic-ingredients", "no-tickets")` — platform-native on each OS, overridable via `NO_TICKETS_HOME=<dir>` which yields `<dir>/.notickets/`).

`--agent` is the only required flag on `session start`. `--model` and all other LLM-context fields are optional and simply omitted from the actor block when not supplied. There are no sentinel values like `"n/a"`.

Default session lifetime: 24 hours from `startedAt`. Configurable via `--max-age-hours N` (hard cap 7 days). After expiry, `no-tickets publish` falls back to credential-based human actor, then to unattributed publish.

Multi-session concurrency: `NO_TICKETS_SESSION_FILE=/path/to/another.json` points at an alternate file. The env var carries an opaque path only — not LLM details.

### `no-tickets publish` actor resolution precedence

```
1. --actor-* flags present                                 → flags win
2. NO_TICKETS_SESSION_FILE env var set                     → read that file
3. <config-dir>/active-session.json present + fresh        → use it
4. session credentials present (no-tickets init was run)   → human actor from creds
5. otherwise                                               → publish without metadata
                                                             (one-time hint on stderr — see below)
```

There is no error branch. Publish always succeeds when the envelope itself is valid; missing actor is a permanent valid state.

### First-publish hint

When publish resolves to no actor (branch 5) **and** the hint marker `<config-dir>/state.json` does not have `firstPublishHintShown: true`, the CLI prints a one-time hint to stderr and sets the flag:

```
Tip: this event was published without an actor. Events without actor metadata
are not attributable in the UI or activity feed.

To attach actor info to every publish in this shell, run once:
  no-tickets session start --agent <your-agent-id>

(This hint shows only once. `no-tickets session end` clears it so it can fire
again next time you're in the no-session state.)
```

The hint is informational only — exit code stays 0 and the event lands normally. The marker file is part of `<config-dir>` alongside `credentials` and `config.json`. **The CLI does not detect harness env vars** (`CLAUDECODE`, `GITHUB_ACTIONS`, etc.) to customise the hint or pre-fill the suggested `--agent`. The hint is deliberately generic — declaration is the caller's choice.

### Directory Structure

```
<config-dir>/                # ProjectDirs::config_dir() (ADR-0002)
                             # ~/.config/no-tickets/ on Linux
                             # ~/Library/Application Support/com.magic-ingredients.no-tickets/ on macOS
                             # %APPDATA%\magic-ingredients\no-tickets\config\ on Windows
├── credentials              # existing — human session token from `no-tickets init` / `init --device`
├── config.json              # existing — project → push-token registry
├── active-session.json      # NEW — atomic file written by `no-tickets session start`
└── state.json               # NEW — small CLI state file; carries firstPublishHintShown flag
```

## Release Criteria

### Functional Requirements

- [ ] `actorSchema`, `eventMetadataSchema`, and types are exported from `@magic-ingredients/no-tickets-schemas`
- [ ] Rust `nt-schemas` crate provides a `validate_metadata()` function with TS-parity issue shapes
- [ ] `no-tickets session start / show / end` subcommands implemented; file is atomically written and stale-detectable
- [ ] `no-tickets publish` resolves actor per the precedence chain; emits `metadata.actor` on every envelope **when an actor resolves**; omits `metadata` from the envelope when none does
- [ ] First-publish hint mechanic: when publish lands on the no-actor branch and the hint marker is unset, print the hint to stderr and set the marker; `session end` clears the marker
- [ ] Server-side `eventEnvelopeSchema` accepts `metadata` as optional — permanently
- [ ] `events.metadata` jsonb column persists actor on rows where the caller declared one; remains `NULL` on rows where they didn't
- [ ] Partial GIN indexes on `metadata.actor.agentId` and `metadata.actor.userId` (matching only the rows that have those values)
- [ ] Read APIs surface `metadata` and accept `?actorType`, `?agentId`, `?userId` filters
- [ ] New endpoint `/v1/projects/:projectId/actors` returns the distribution of actors in the project (excludes events with NULL metadata)
- [ ] Board renders an actor pill on event cards **that have an actor**; unattributed events render without a pill (no placeholder)
- [ ] Activity feed is filterable by actor (filter respects "actor present" semantics)
- [ ] Existing GDPR user-deletion job extends to redact `metadata.actor.userId` / `email` on events where the user is the actor
- [ ] `ai.*` event-type `data` schemas no longer duplicate `agentId` / `model` / `provider` for events published *during a declared session* (those move to metadata)
- [ ] Per-language wrappers (TS / Python / Go) spawn `no-tickets session start` at init **when their constructor was given actor config**; spawn nothing otherwise

### Usability Requirements

- [ ] Zero-config happy path for human pushes: after `no-tickets init`, `no-tickets publish` works with no actor flags and stamps the human actor automatically
- [ ] Zero-extra-config happy path for agents: after `no-tickets session start --agent X`, every `no-tickets publish` stamps the actor automatically
- [ ] Zero-config happy path for unattributed publishes: `no-tickets publish` with no session and no credentials succeeds, prints a one-time hint, then stays silent
- [ ] `no-tickets session show` reports the active session in JSON, including expiry warning when stale; reports `{"active": false}` when no session is set
- [ ] Actor pill renders without truncation on standard board card widths
- [ ] Activity-feed actor filter is keyboard-navigable

### Technical Requirements

- [ ] TS↔Rust validator parity test in `nt-schemas` covers all actor variants (human, agent), all required-field violations, type-discrimination
- [ ] `active-session.json` and `state.json` writes are atomic (temp + rename)
- [ ] `no-tickets session` subcommands are covered by `assert_cmd`-driven integration tests
- [ ] `no-tickets publish` resolution precedence covered by table-driven tests (flags > `NO_TICKETS_SESSION_FILE` > active file > credentials > unattributed-with-hint)
- [ ] First-publish hint is shown at most once between `session end` boundaries; tests cover the marker write + clear paths
- [ ] Stryker mutation review clean on `no-tickets session` and actor-resolution paths
- [ ] Schemas-bundle GH Release artefact (sister fix Task 6/7) includes `eventMetadataSchema` as a top-level entry

## Success Metrics (KPIs)

Actor attribution is opt-in, so coverage % is **informational only — no target**. The metrics below characterise *whether the feature is useful to people who chose to use it*, not whether the population was driven onto it.

- **Actor coverage (informational)**: % of events in `events` table with non-null `metadata.actor`. Reported, not targeted.
- **Agent attribution depth (informational)**: % of `agent` actors that include `model` AND `sessionId`. Higher → richer attribution among opt-in users.
- **`ai.*` payload size reduction**: payload byte-size shrink for `ai.completion.recorded.v1` and `ai.task.completed.v1` after duplicated identity fields move out of `data`. Target: 15–25% smaller for events from declared sessions.
- **Cross-domain agent query latency**: P95 of `SELECT … FROM events WHERE metadata->'actor'->>'agentId' = $1 AND project_id = $2` on a 1M-row table. Target: < 50 ms with the partial index. (Index covers the opt-in subset.)
- **Hint dismissal rate**: % of installs whose `state.json` shows `firstPublishHintShown: true`. Indicates the hint is reaching real callers without being annoying.

## Constraints and Dependencies

### Technical Constraints

- Actor attribution is opt-in. There is no breaking change at any phase; `metadata` is permanently optional on the wire and in the database.
- `metadata` lives in jsonb, not normalised columns. Reducer queries use jsonb path expressions, not joins.
- Stale-session detection is timestamp-based, not pid-liveness-based. PID is recorded but not currently checked (cross-platform pid validation adds complexity for marginal benefit).
- TS↔Rust validator parity is the contract; if shapes diverge, parity tests fail CI.
- Storage paths resolve via `paths::config_dir()` per ADR-0002 (platform-native via `directories::ProjectDirs`); `NO_TICKETS_HOME` is the only home-directory override.

### Dependencies

- **`cross-platform-cli-binary` fix (✅ completed 2026-05-20)**: Task 4 (full CLI port) provides the `clap` derive scaffolding for the `session` subcommand group. Task 26 (structured-error contract on stderr + exit codes) provides the error envelope shape (note: no exit-5 case from this PRD — the resolution chain ends in a soft hint, not an error). Task 5 (MCP server port) carries actor wiring on its publish tool. ADR-0002 provides the platform-native config-dir resolution this PRD relies on.
- **`headless-init-device-code` fix (not_started)**: the human-actor fallback in the precedence chain only works on hosts where `no-tickets init` can run. The device-code flow is what makes `init` work in CI / sandbox / SSH / containers, so headless agents that *want* the human-actor fallback need this to land first. Not a hard gate — agents that explicitly declare via `session start` don't need credentials at all — but the docs should cross-reference both paths.
- **Sister fix `client-roadmap-server-prerequisites`**: the JSON Schema bundle in GH Releases needs to include `eventMetadataSchema`. One-line addition to `build-json-schema-bundle.ts` when this PRD lands.
- **Permissions-broadening commit (2026-05-13)**: this PRD adds the *option* of an audit trail, complementing that broadening. The pair stays defensible because callers who care can opt into attribution declaratively; callers who don't aren't worse off than they were before the broadening.

### Known Limitations

- **No environment-based actor inference.** Publishing without `session start`, without `--actor-*` flags, and without `no-tickets init` credentials succeeds with `metadata` omitted. The CLI does not sniff `CLAUDECODE`, `GITHUB_ACTIONS`, `CI`, or any other harness env var to synthesise an actor block. Reason: env vars are forgeable; auto-filling them would make the audit trail agree with anything a process claims, defeating the purpose of having one. Detection is also *not* used to customise the first-publish hint — the hint is deliberately generic. Declaration is always the caller's explicit choice.
- **No `system` actor variant in v1.** CI bots, cron jobs, and migrations model themselves as `agent` with `agentId: 'github-actions'` / `'cron'` / `'migration'` and **no `model` field at all** (the field is optional and omitted when meaningless). No sentinel values like `"n/a"` anywhere in the design. If a `system` variant proves useful in practice we add it as a v1.1 discriminated-union branch; until then, `agent` with just `agentId` is the shape for non-LLM systems.
- **No top-level `metadata.version` field.** Discriminated unions handle additions; a breaking shape change would version per `actor.type` (e.g. `agent.v2`) the same way event types are versioned.
- **No remote validation on `no-tickets session start`** in v1. The actor block validates locally; if the server rejects it at first publish, that's how the harness learns about a typo.
- **GDPR redaction is point-in-time, not cryptographic.** A deleted user's past events are stamped with `userId: null` but the event itself remains. Cryptographic erasure (key-shred per-user) is a separate concern.
- **No per-token actor restriction.** A push token authorised for project X can still emit any `actor.agentId`. Trust is enforced at the project / token boundary, not the actor identity boundary. The actor block is for accountability and observability, not access control.
- **Coverage is forever incomplete.** Phase 3 does not flip `metadata` to required. The trade-off: opt-in keeps the feature trustworthy (no surveillance) and frictionless (no breaking change), at the cost of a permanent class of unattributable events. We accept that.
