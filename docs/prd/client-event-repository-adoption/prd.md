---
id: client-event-repository-adoption
title: "Client Event Repository Adoption — Envelopes, publish(), Discovery"
version: 1.2.1
status: not_started
created: 2026-04-27
updated: 2026-05-07
author: Andy Richardson
---

# Client Event Repository Adoption

## Purpose and Goals

The `no-tickets-service` rewrite (`event-repository-foundation` PRD plus the `domain-*` PRDs) reframes the platform as a generic event repository: every signal is a typed event in an append-only store, validated against a code-first schema registry. This PRD is the client side of that rewrite — the SDK, CLI, and MCP server changes that let agents and integrations actually publish events into the new substrate.

The decision governing this PRD is captured in [ADR-0001](../../adr/0001-schema-source-of-truth-and-discovery.md): **the SDK owns wire-format envelopes; the server registry owns the domain catalogue; discovery is `GET /v1/admin/event-types`**. This PRD implements that decision.

The sister PRD on the server (`single-events-endpoint-and-product-domain` in `no-tickets-service`) consolidates the server-side wire format around `POST /v1/events` accepting an `Event[]` array body — replacing the transitional `/v1/snapshots` endpoint. The CLI verb is renamed from `push` to `publish` to match the wire-format contract. Discussion in v1.1 of this PRD reflects that consolidation; the original v1.0 referenced `emitEvent` and a single-event endpoint, both superseded.

We are explicitly accepting no v1 backwards compatibility, mirroring the foundation PRD. Push v2, the existing `Push` zod, the legacy schema bundle in `src/core/types.ts` and `src/core/schemas.ts` — all superseded. The completed `push-schemas`, `push-cli`, and `push-token-cli` features in `no-tickets-client` are superseded by features in this PRD when this work lands.

The legacy `nt push` CLI command targets `POST /v1/push`, which the new server has already removed — every `nt push` is currently a 404 against the new substrate. There's no degradation window from this PRD's rollout; the CLI is already broken and this work is the fix.

### Goals

- Single publish primitive at the wire — `POST /v1/events` with `Event[]` body. SDK mirrors with `publish(events: Event[])`.
- User-facing surfaces (CLI, MCP) reshape the array primitive into single-event calls because that matches how shells and agent tool loops naturally interact. Batch is an explicit mode (`nt publish --batch <file.jsonl>`), not the default.
- Envelopes only in the SDK; domain payload typing is opt-in, not required.
- Discovery built into the CLI and MCP server, so agents pick the right type per task without curated prompts.
- Registry conformance with no SDK-side schema duplication.

### MVP / release slice

All five features ship together as one major SDK release. The breaking removals (push v2 schemas, `nt push`, `client.push()`) and the new surface land atomically; there is no partial rollout. Major version bump on `@magic-ingredients/no-tickets` is required.

### Feature DAG

The "Order is implementation order" framing in earlier drafts overstated serialisation. Real dependency graph:

```
F1 ─┬─→ F2 ─┬─→ F4
    └─→ F3 ─┘   F5
```

F1 (envelopes) blocks everything. F2 (transport) and F3 (introspection) are independent — they share F1's schemas but not each other. F4 (CLI) and F5 (MCP) both consume F2 and F3, and are independent of each other. Implementation can parallelise F2 || F3, then F4 || F5.

### Non-goals (deferred)

- Codegenned `-types` companion package for typed domain payloads — out of scope; tracked separately, likely shipped from `no-tickets-service`.
- Polyglot SDKs (Python, Go) — TS only in v1; non-TS consumers use raw HTTP and JSON Schema from the introspection endpoint.
- Migration tooling for legacy push payloads — none, by design.
- Tile / Mission Control extensibility from the SDK side — server-only concern.
- **Subject-type discovery** (`nt subject types`, MCP `list_subject_types`) — deferred to a follow-up PRD. Event-type discovery ships in v1; subject-type discovery is asymmetric for now. Documented gap.
- **MCP batch publish** (`publish_events` plural) — no agent batch use case justified yet. Add later if one appears; non-breaking addition.

## User Needs

### Target Audience

- **Agent authors** (Claude Code, Cursor, Windsurf, custom agents) — emit typed events from their own code paths or via MCP tools.
- **CI runners and integrations** — emit events from build pipelines without a TS SDK, using JSON Schema from the introspection endpoint.
- **CLI users (developers, ops)** — discover what types exist, describe their schemas, send arbitrary events from the shell.
- **tiny-brain** — emits events for tracked PRD/fix work; consumes envelope types from the SDK.

### User Stories

1. As an agent author, I want to ask the platform "what event types can I send?" at task time so that I pick the right type without hand-curated prompts.
2. As a CLI user, I want `nt event list` to show me types I have permission to write so that the listing reflects reality, not the global catalogue.
3. As a CLI user, I want `nt event describe engineering.deploy.completed.v1` to print a payload example and the required fields so that I can craft a valid send by hand.
4. As an agent author, I want my MCP server to expose `list_event_types` and `describe_event_type` tools so that the agent introspects the registry without a curated prompt.
5. As a developer, I want to typo a type id and have the CLI suggest the closest match so that I recover from mistakes in one keystroke.
6. As a CI integration, I want to validate my payload against a JSON Schema fetched from the platform so that I fail fast before the server rejects.
7. As a developer, I want the SDK to never lie about what events exist — if something isn't in the server registry, the SDK doesn't claim it does.

## Features and Functionality

Five features. See the Feature DAG above for parallelisation; "feature numbers" reflect dependency order, not strict serialisation.

### Feature 1: Envelope Schemas + SDK Surface Reset
**File**: [features/envelope-schemas.md](features/envelope-schemas.md)
**Status**: not_started
**Description**: Define and export `Event`, `Source`, `Subject`, `Interaction`, `Session`, `Actor`, and the type-ID grammar as the only domain-shaped schemas the SDK ships. Remove push v2 schemas (`Push`, `WorkSchema`, `EngineeringSchema`, `ProductSchema`, `CodeQualitySchema`, etc.) from `src/core/types.ts` and `src/core/schemas.ts`. SDK surface shrinks to wire-format primitives.

### Feature 2: publish() + Subjects + Interactions HTTP Client
**File**: [features/publish-client.md](features/publish-client.md)
**Status**: not_started
**Description**: `publish(events: Event[])` against `POST /v1/events` with array body. Each event carries `{ type, data, source, subject?, occurredAt?, parentEventId?, traceId?, dedupeKey? }`. Single event = `[oneEvent]`. `subjects.create/list/get` against `/v1/subjects`. `runInteraction(id, input)` against `/v1/interactions/:id`. `data` is pass-through; SDK does not validate domain payloads (server does). `source` is auto-filled by the SDK based on entry surface.

### Feature 3: Registry Introspection + Caching
**File**: [features/registry-introspection.md](features/registry-introspection.md)
**Status**: not_started
**Description**: `client.events.list()` and `client.events.describe(typeId)` against `GET /v1/admin/event-types`. Permission-scoped responses. Cache at `<cwd>/.notickets/.cache/registry-<hash>.json` (project-local) with fallback to `~/.notickets/.cache/registry-<hash>.json` when no project root is detected. Cache shape carries `{ version, etag, fetchedAt, types }`. ETag-driven refresh. Cache reads always synchronous; refresh fires async with a short bounded wait (default 200ms) so first-invocation drift detection works. Stale-cache warning above configurable threshold (default 14 days).

### Feature 4: nt publish / nt event / nt subject / nt action CLIs
**File**: [features/registry-aware-cli.md](features/registry-aware-cli.md)
**Status**: not_started
**Description**: `nt publish <type-id> --data <json|@file|->` for single events, `nt publish --batch <file.jsonl>` for bulk publishes (validates against cached JSON Schema before sending; suggests fuzzy matches on unknown type). `nt event list` (grouped by domain, scoped by permission), `nt event describe <type-id>` (human-readable schema with synthesised example). `nt subject create/list/get` (subject promotion + lookup; subject-type discovery is deferred — see Non-goals), `nt action <interaction-id> --input <json>`. The legacy `nt push` is removed — `nt publish` is its replacement. Subject references are passed via separate `--subject-type <t> --subject-id <id>` flags (not a colon-encoded combined value). Drift-notification one-liner on routine commands when registry diff is non-empty.

### Feature 5: MCP Discovery Tools
**File**: [features/mcp-discovery-tools.md](features/mcp-discovery-tools.md)
**Status**: not_started
**Description**: Expose `list_event_types`, `describe_event_type`, `publish_event` (singular — single event per call), `run_interaction`, `create_subject` as MCP tools. The MCP surface is single-event-shaped because agent loops naturally produce one event per reasoning step; the array shape lives at the wire and SDK layers, not in agent tool calls. No `publish_events` (plural, batch) tool ships in v1 — add only when a real agent batch use case is demonstrated. Wraps the same client primitives as the CLI.

## Design and User Experience

### Wire-format primitives owned by the SDK

```ts
// Event envelope
type Event<T = unknown> = {
  readonly type: string;       // e.g. 'engineering.deploy.completed.v1'
  readonly data: T;            // domain payload, validated by server registry
  readonly source: Source;     // mandatory; SDK auto-fills based on entry surface
  readonly subject?: SubjectRef;
  readonly occurredAt?: string; // ISO-8601; defaults server-side to now
  readonly parentEventId?: string;
  readonly traceId?: string;
  readonly dedupeKey?: string;
};

type Source = {
  readonly name: string;       // 'cli' | 'mcp' | 'ci' | 'cron' | 'integration' | 'sdk'
  readonly sdkVersion: string; // version of @magic-ingredients/no-tickets, SDK auto-fills
  readonly version?: string;   // version of the named producer (when distinct from SDK)
  readonly attributes?: Readonly<Record<string, string | number | boolean>>;
};

type SubjectRef = { readonly type: string; readonly id: string };

// Type-ID grammar
// Regex: ^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*){2}\.v[1-9]\d*$
// Lowercase only, underscores allowed within segments, version is v1+ (no v0, no leading zeros).
type TypeId = string;
parseTypeId(s): { domain, entity, action, version } | null
```

#### Source semantics

`source` answers *"how did this event enter the platform?"* — the entry channel and the version of the thing producing it. Distinct from:
- `type` (what kind of event)
- `subject` (what entity it's about)
- the auth token (who the caller is)
- `traceId` / `parentEventId` (causal lineage)

**Required, but rarely set explicitly.** The SDK auto-fills `name` and `sdkVersion` based on entry surface; callers almost never pass `source` directly.

**Auto-fill defaults per surface:**
- `nt publish` → `{ name: 'cli', sdkVersion, version: <cli-version> }`
- MCP server → `{ name: 'mcp', sdkVersion, attributes: { client: <transport-detected>, clientVersion: <if-available> } }` — server fills, not the agent (agents must not be trusted to claim their runtime)
- Direct SDK use → `{ name: 'sdk', sdkVersion }`
- CI auto-detect (via `detectAgent()`) → `{ name: 'ci', sdkVersion, attributes: { provider, runId, workflow } }` for known providers (GitHub Actions, GitLab, Circle, ...)
- Cron — caller passes `{ name: 'cron', attributes: { jobId, schedule? } }`; no auto-detection

**Override semantics:** when a caller passes `source` explicitly, it **merges** with auto-detected source (caller's fields win). Lets callers add `attributes.feature: 'experimental'` without losing the SDK-filled name and version.

**`attributes` cookbook** (documented conventions, not enforced by schema):
- All sources: `attributes.machine` (opt-in via `NO_TICKETS_INCLUDE_MACHINE=1`) — hashed hostname (per-installation salt; never raw hostname, which is PII)
- `name: 'ci'`: `attributes.provider`, `attributes.runId`, `attributes.workflow`
- `name: 'cron'`: `attributes.jobId`, `attributes.schedule`
- `name: 'mcp'`: `attributes.client`, `attributes.clientVersion`
- `name: 'integration'`: `attributes.integration` (the integration's name)

Anything else can go in `attributes` too — it's free-form. The cookbook covers the common patterns so analytics queries are predictable; nothing stops callers adding their own keys.

**Trust model:** `source` is self-reported telemetry. Server uses it for analytics, debugging, deprecation tracking. Server **must not** use `source` for authorisation decisions — that's what auth tokens are for.

### Discovery flow at the CLI

```
$ nt event list --domain engineering
engineering.service.created.v1
engineering.service.updated.v1
engineering.deploy.started.v1
engineering.deploy.completed.v1
engineering.deploy.rolled_back.v1
engineering.health.probe_requested.v1   (interaction)
engineering.health.checked.v1
engineering.incident.fired.v1
engineering.incident.acknowledged.v1
engineering.incident.resolved.v1

$ nt event describe engineering.deploy.completed.v1
engineering.deploy.completed.v1
  domain:    engineering
  retention: 90 days
  dedupe:    (service_id, sha)

  required:
    service_id: string
    sha:        string
    env:        'production' | 'staging' | 'dev'
    duration_ms: number
  optional:
    rolled_back: boolean

  example payload:
    { "service_id": "api", "sha": "abc1234", "env": "production", "duration_ms": 42000 }
```

### Discovery flow for agents (MCP)

```
agent → list_event_types({ domain: 'engineering' })
agent → describe_event_type({ id: 'engineering.deploy.completed.v1' })
agent → publish_event({
  type: 'engineering.deploy.completed.v1',
  data: { ... },
  subject: { type: 'engineering_service', id: 'api' }
})
```

The agent never has the registry baked into its prompt; it asks every time. `publish_event` accepts one event per call — agent reasoning loops naturally produce one event per phase, so per-event publish gives per-event errors and per-event recovery. Source is filled by the MCP server, not the agent.

Why singular: the array shape lives at the wire and SDK layers (atomic batch transactions, bulk imports, end-of-session bundles from integrations like tiny-brain). User-facing surfaces — CLI invocations, agent tool calls — reshape that primitive into single-shot calls because that matches how their audiences naturally interact. Forcing arrays into MCP would make agents either defer emission until they had a batch (fighting the loop) or call with a one-element array (cognitive overhead with no upside).

### Wire format on the wire

```http
POST /v1/events
Authorization: Bearer <push-token>
Content-Type: application/json

[
  {
    "type": "ai.completion.recorded.v1",
    "data": { "callId": "...", "...": "..." },
    "source": { "name": "integration", "sdkVersion": "0.5.0", "version": "0.4.2", "attributes": { "integration": "tiny-brain" } },
    "occurredAt": "2026-05-06T10:30:00Z",
    "traceId": "session-abc"
  },
  {
    "type": "ai.review.completed.v1",
    "data": { "...": "..." },
    "source": { "name": "integration", "sdkVersion": "0.5.0", "version": "0.4.2", "attributes": { "integration": "tiny-brain" } },
    "occurredAt": "2026-05-06T10:30:01Z",
    "traceId": "session-abc"
  }
]
```

Response: `{ ingested, deduped, ids }`. Per-event 422s carry `batchIndex` for caller-side identification. Response shape owned by the server PRD; SDK conforms.

#### Retry policy

`POST /v1/events` is **not retried** in v1. Caller-supplied `dedupeKey` is honoured server-side (per the server PRD), which means callers who want at-least-once semantics across their own retries can attach a `dedupeKey` and re-publish safely. Idempotent reads (`subjects.list/get`, `events.list/describe`) get bounded retries on 5xx.

## Release Criteria

### Functional Requirements

- [ ] SDK ships envelope schemas only; no domain payloads in `src/core/`.
- [ ] `publish` round-trips against the server's `POST /v1/events` for at least one registered type (`ai.completion.recorded.v1` smoke).
- [ ] `Source` is required on every event; SDK auto-fills `name` and `sdkVersion` per entry surface; callers may override and overrides merge with auto-detected fields.
- [ ] `nt publish <type-id> --data` validates against cached JSON Schema before send (best-effort; server is authoritative); unknown type prints a fuzzy-match suggestion.
- [ ] `nt publish --batch <file.jsonl>` publishes every line as one event in a single request.
- [ ] `nt event list` returns permission-scoped types; cache survives offline reads after first fetch.
- [ ] `nt event describe` renders required/optional fields and a synthesised example from the JSON Schema.
- [ ] `runInteraction` returns event IDs and fires the registered handler server-side.
- [ ] MCP server exposes `list_event_types`, `describe_event_type`, `publish_event` (singular), `run_interaction`, `create_subject`. No `publish_events` (plural) tool ships.
- [ ] tiny-brain integration cutover lives in the tiny-brain repo (this PRD ships the SDK surface tiny-brain consumes; the cutover work itself is tracked there, not here).
- [ ] Legacy `nt push` command is removed; `client.push()` removed from the SDK; Push v2 zod schemas (`Push`, `WorkSchema`, `EngineeringSchema`, `ProductSchema`, `CodeQualitySchema`) deleted from `src/core/`.
- [ ] Major version bump on `@magic-ingredients/no-tickets` (breaking removals).

### Technical Requirements

- [ ] Strict TypeScript; no `any` in the SDK or CLI.
- [ ] Envelope zod schemas have no `.refine()` so the JSON Schema export is faithful (consistent with the rule applied to server-side registered types).
- [ ] Type-ID grammar enforced via regex `^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*){2}\.v[1-9]\d*$`.
- [ ] Cache file carries `"version": 1` field; corrupt-cache recovery wipes and re-fetches.
- [ ] All new HTTP calls go through a single client module that handles auth, retries (per Retry policy above), and structured error mapping.
- [ ] SDK has built-in observability: per-publish trace log at debug level (request id, event count, type ids), structured warnings on retry, debug log on async refresh failures.
- [ ] `sdkVersion` resolved at build time (no `require('./package.json')` runtime call — breaks ESM-only consumers).

### Usability Requirements

- [ ] First-time discovery from a clean checkout (no cache, fresh auth) takes < 2s wall time.
- [ ] `nt event list` works fully offline once cached.
- [ ] Drift notification is at most one line, never blocks commands beyond the bounded async-refresh wait (default 200ms), and goes away once the user runs `nt event list`.
- [ ] MCP tool descriptions are short enough to not bloat agent context (≤ 60 words each).

## Success Metrics

- Time from "agent author wants to emit a new domain event type" to "first event lands": no SDK release required.
- Cache hit rate on `nt event list` after first fetch: > 95% across normal usage.
- Number of "unknown type" errors that resulted in successful retry after the fuzzy-match suggestion: tracked on the server side; aim for > 80%.
- Lines of domain-schema code in the SDK after rollout: ~0. The SDK is wire-format only.

## Constraints and Dependencies

### Technical Constraints

- Cannot start before `event-repository-foundation` Feature 1 Tasks 5 (smoke event type) and 6 (introspection route) ship — those are the substrate this PRD plugs into.
- Cannot finish before at least one domain PRD (`domain-ai-telemetry`) lands — without registered domain types, end-to-end validation has nothing to send.
- Cache file lives at `<cwd>/.notickets/.cache/registry-<hash>.json` (project-local), with fallback to `~/.notickets/.cache/registry-<hash>.json` when the working directory has no `.notickets/`. The hash key includes the configured server URL so multi-tenant local development doesn't cross streams. `.notickets/.cache/` should be gitignored (instructions in README).
- Multi-process race on cache writes: two concurrent invocations can both fire refreshes; atomic temp+rename prevents file corruption, but a slower-but-newer response can lose to a faster-but-older one. ETag mostly mitigates; documented behaviour rather than fixed.
- Auth scoping for `/v1/admin/event-types`: push tokens and session tokens must both work for introspection, otherwise CI runners with only push tokens cannot validate locally. Confirmed contract with the server PRD.

### Dependencies

- **`event-repository-foundation` PRD** — required (Features 1, 2, 4).
- **`domain-ai-telemetry` PRD** — required for end-to-end validation; AI agents are the canonical first emitters.
- **Existing `no-tickets-client` PRD** features 1 (CLI auth), 2 (push token CLI) — auth substrate stays the same.

### Known Limitations

- TS consumers wanting typed domain payloads must wait for the codegenned `-types` package (out of scope here).
- Refinements on registered server-side schemas are banned by ADR-0001; cross-field validations move to reducers. If this proves too restrictive, revisit the ADR.
- Permission-scoped listing depends on the server's permission model being in place; if permission-scoping ships late, `nt event list` will show the global catalogue temporarily, with a documented caveat.
- Subject-type discovery is asymmetric vs event-type discovery in v1 — `nt subject create --type <t>` requires the user to know `<t>` exists out of band. Tracked in a follow-up PRD.
- Client-side JSON Schema validation is **best-effort**, not authoritative: a stale cache may pass a payload the server rejects (e.g., schema added a new required field). Server validation is the source of truth.
- `POST /v1/events` is not retried on 5xx (see Retry policy). Operationally this means a failed publish during a server blip surfaces to the caller; the caller decides whether to retry. Callers who want at-least-once semantics across their own retries attach a `dedupeKey` and re-publish — the server honours `dedupeKey` when present and falls back to per-type registered dedupe strategies otherwise.

## Related work

- [ADR-0001: Schema source of truth — wire/domain split with server registry](../../adr/0001-schema-source-of-truth-and-discovery.md)
- `event-repository-foundation` PRD (`no-tickets-service`)
- `single-events-endpoint-and-product-domain` PRD (`no-tickets-service`) — sister PRD on the server side. Owns the `POST /v1/events` array-body endpoint, the legacy `/v1/snapshots` removal, and the legacy schema drop. This PRD ships *after* the server endpoint exists; cross-repo coordination is the user's responsibility.
- `domain-*` PRDs (`no-tickets-service`)
- Superseded on landing: `no-tickets-client` features `push-schemas`, `push-cli`, `push-token-cli`
