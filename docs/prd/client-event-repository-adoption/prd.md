---
id: client-event-repository-adoption
title: "Client Event Repository Adoption — Envelopes, publish(), Discovery"
version: 1.1.0
status: not_started
created: 2026-04-27
updated: 2026-05-06
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

- Single publish primitive — `publish(events: Event[])` — for every domain, replacing `push`. Always an array; single event is `[oneEvent]`.
- Envelopes only in the SDK; domain payload typing is opt-in, not required.
- Discovery built into the CLI and MCP server, so agents pick the right type per task without curated prompts.
- Registry conformance with no SDK-side schema duplication.

### Non-goals (deferred)

- Codegenned `-types` companion package for typed domain payloads — out of scope; tracked separately, likely shipped from `no-tickets-service`.
- Polyglot SDKs (Python, Go) — TS only in v1; non-TS consumers use raw HTTP and JSON Schema from the introspection endpoint.
- Migration tooling for legacy push payloads — none, by design.
- Tile / Mission Control extensibility from the SDK side — server-only concern.

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

Five features. Order is implementation order.

### Feature 1: Envelope Schemas + SDK Surface Reset
**File**: [features/envelope-schemas.md](features/envelope-schemas.md)
**Status**: not_started
**Description**: Define and export `Event`, `Subject`, `Interaction`, `Session`, `Actor`, and the type-ID grammar as the only domain-shaped schemas the SDK ships. Remove push v2 schemas (`Push`, `WorkSchema`, `EngineeringSchema`, `ProductSchema`, `CodeQualitySchema`, etc.) from `src/core/types.ts` and `src/core/schemas.ts`. SDK surface shrinks to wire-format primitives.

### Feature 2: publish() + Subjects + Interactions HTTP Client
**File**: [features/emit-event-client.md](features/emit-event-client.md)
**Status**: not_started
**Description**: `publish(events: Event[])` against `POST /v1/events` with array body. Each event carries `{ type, data, subject?, source?, occurredAt?, parentEventId?, traceId?, dedupeKey? }`. Single event = `[oneEvent]`. `subjects.create/list/get` against `/v1/subjects`. `runInteraction(id, input)` against `/v1/interactions/:id`. `data` is pass-through; SDK does not validate domain payloads (server does).

### Feature 3: Registry Introspection + Caching
**File**: [features/registry-introspection.md](features/registry-introspection.md)
**Status**: not_started
**Description**: `client.events.list()` and `client.events.describe(typeId)` against `GET /v1/admin/event-types`. Permission-scoped responses. Cache at `.notickets/registry-cache.json` with `{ etag, fetchedAt, types }` shape; ETag-driven refresh. Cache reads always synchronous; refresh always asynchronous. Stale-cache warning above configurable threshold (default 14 days).

### Feature 4: nt publish / nt event / nt subject / nt action CLIs
**File**: [features/registry-aware-cli.md](features/registry-aware-cli.md)
**Status**: not_started
**Description**: `nt publish <type-id> --data <json|@file|->` for single events, `nt publish --batch <file.jsonl>` for bulk publishes (validates against cached JSON Schema before sending; suggests fuzzy matches on unknown type). `nt event list` (grouped by domain, scoped by permission), `nt event describe <type-id>` (human-readable schema with synthesised example). `nt subject create/list/get`, `nt action <interaction-id> --input <json>`. The legacy `nt push` is removed — `nt publish` is its replacement. Drift-notification one-liner on routine commands when registry diff is non-empty.

### Feature 5: MCP Discovery Tools
**File**: [features/mcp-discovery-tools.md](features/mcp-discovery-tools.md)
**Status**: not_started
**Description**: Expose `list_event_types`, `describe_event_type`, `publish_events`, `run_interaction`, `create_subject` as MCP tools. `publish_events` accepts an `events: Event[]` array argument matching the wire format. Wraps the same client primitives as the CLI. This is the load-bearing UX for agents — discovery and publishing in the agent's tool surface.

## Design and User Experience

### Wire-format primitives owned by the SDK

```ts
// Event envelope
type Event<T = unknown> = {
  readonly type: string;       // e.g. 'engineering.deploy.completed.v1'
  readonly data: T;            // domain payload, validated by server registry
  readonly subject?: SubjectRef;
  readonly source: string;     // 'cli' | 'ci' | 'mcp' | 'integration:<name>' | ...
  readonly occurredAt?: string; // ISO-8601; defaults server-side to now
  readonly parentEventId?: string;
  readonly traceId?: string;
  readonly dedupeKey?: string;
};

type SubjectRef = { readonly type: string; readonly id: string };

// Type-ID grammar
type TypeId = `${string}.${string}.${string}.v${number}`;
parseTypeId(s): { domain, entity, action, version } | null
```

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
agent → publish_events({
  events: [
    { type: 'engineering.deploy.completed.v1', data: { ... }, subject: { type: 'engineering_service', id: 'api' } }
  ]
})
```

The agent never has the registry baked into its prompt; it asks every time. `publish_events` always takes an array — single event is `events: [oneEvent]`, batch is `events: [a, b, c, …]`. Same shape as the wire format the SDK and CLI use.

### Wire format on the wire

```http
POST /v1/events
Authorization: Bearer <push-token>
Content-Type: application/json

[
  { "type": "ai.completion.recorded.v1", "data": { "callId": "...", ... }, "occurredAt": "2026-05-06T10:30:00Z", "traceId": "session-abc" },
  { "type": "ai.review.completed.v1",    "data": { ... },                  "occurredAt": "2026-05-06T10:30:01Z", "traceId": "session-abc" }
]
```

Response: `{ ingested, deduped, ids }`. Owned by the server PRD; the SDK conforms to it.

## Release Criteria

### Functional Requirements

- [ ] SDK ships envelope schemas only; no domain payloads in `src/core/`.
- [ ] `publish` round-trips against the server's `POST /v1/events` for at least one registered type (`ai.completion.recorded.v1` smoke).
- [ ] `nt publish <type-id> --data` validates against cached JSON Schema before send; unknown type prints a fuzzy-match suggestion.
- [ ] `nt publish --batch <file.jsonl>` publishes every line as one event in a single request.
- [ ] `nt event list` returns permission-scoped types; cache survives offline reads after first fetch.
- [ ] `nt event describe` renders required/optional fields and a synthesised example from the JSON Schema.
- [ ] `runInteraction` returns event IDs and fires the registered handler server-side.
- [ ] MCP server exposes `list_event_types`, `describe_event_type`, `publish_events`, `run_interaction`, `create_subject`.
- [ ] tiny-brain pushes for tracked PRD/fix work publish `ai.completion.recorded.v1` / `ai.review.completed.v1` / `ai.task.completed.v1` directly via `publish`, not via the legacy push payload.
- [ ] Legacy `nt push` command is removed; `client.push()` removed from the SDK; Push v2 zod schemas (`Push`, `WorkSchema`, `EngineeringSchema`, `ProductSchema`, `CodeQualitySchema`) deleted from `src/core/`.

### Technical Requirements

- [ ] Strict TypeScript; no `any` in the SDK or CLI.
- [ ] Envelope zod schemas have no `.refine()` so the JSON Schema export is faithful (consistent with the rule applied to server-side registered types).
- [ ] Cache file format documented and versioned; corrupt-cache recovery wipes and re-fetches.
- [ ] All new HTTP calls go through a single client module that handles auth, retries, and structured error mapping.

### Usability Requirements

- [ ] First-time discovery from a clean checkout (no cache, fresh auth) takes < 2s wall time.
- [ ] `nt event list` works fully offline once cached.
- [ ] Drift notification is at most one line, never blocks commands, and goes away once the user runs `nt event list`.
- [ ] MCP tool descriptions are short enough to not bloat agent context.

## Success Metrics

- Time from "agent author wants to emit a new domain event type" to "first event lands": no SDK release required.
- Cache hit rate on `nt event list` after first fetch: > 95% across normal usage.
- Number of "unknown type" errors that resulted in successful retry after the fuzzy-match suggestion: tracked on the server side; aim for > 80%.
- Lines of domain-schema code in the SDK after rollout: ~0. The SDK is wire-format only.

## Constraints and Dependencies

### Technical Constraints

- Cannot start before `event-repository-foundation` Feature 1 Tasks 5 (smoke event type) and 6 (introspection route) ship — those are the substrate this PRD plugs into.
- Cannot finish before at least one domain PRD (`domain-ai-telemetry`) lands — without registered domain types, end-to-end validation has nothing to send.
- Cache file location (`.notickets/registry-cache.json`) collides with the user's `.notickets/` directory if they're using the local file format — needs a subdirectory or `.notickets/.cache/` to avoid confusion.

### Dependencies

- **`event-repository-foundation` PRD** — required (Features 1, 2, 4).
- **`domain-ai-telemetry` PRD** — required for end-to-end validation; AI agents are the canonical first emitters.
- **Existing `no-tickets-client` PRD** features 1 (CLI auth), 2 (push token CLI) — auth substrate stays the same.

### Known Limitations

- TS consumers wanting typed domain payloads must wait for the codegenned `-types` package (out of scope here).
- Refinements on registered server-side schemas are banned by ADR-0001; cross-field validations move to reducers. If this proves too restrictive, revisit the ADR.
- Permission-scoped listing depends on the server's permission model being in place; if permission-scoping ships late, `nt event list` will show the global catalogue temporarily, with a documented caveat.

## Related work

- [ADR-0001: Schema source of truth — wire/domain split with server registry](../../adr/0001-schema-source-of-truth-and-discovery.md)
- `event-repository-foundation` PRD (`no-tickets-service`)
- `single-events-endpoint-and-product-domain` PRD (`no-tickets-service`) — sister PRD on the server side. Owns the `POST /v1/events` array-body endpoint, the legacy `/v1/snapshots` removal, and the legacy schema drop. This PRD ships *after* the server endpoint exists; cross-repo coordination is the user's responsibility.
- `domain-*` PRDs (`no-tickets-service`)
- Superseded on landing: `no-tickets-client` features `push-schemas`, `push-cli`, `push-token-cli`
