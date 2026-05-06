---
adr_number: 1
title: "Schema source of truth: wire/domain split with server registry"
date: 2026-04-27
status: proposed
supersedes: null
superseded_by: null
tags: [architecture, schemas, sdk, registry, discovery]
decision_makers: [Andy Richardson]
---

# ADR-0001: Schema source of truth — wire/domain split with server registry

## Status

Proposed

## Context

The no-tickets platform is being reframed (per `event-repository-foundation` PRD in `no-tickets-service`) as a generic event repository: every signal becomes a typed event in a single append-only `events` table, validated against a code-first schema registry. Domain modules (AI, product, engineering, scm, support, standup, nightly, otel, flags) plug in by registering event types — no per-domain migrations.

This forces a question the current codebase doesn't have a clean answer to: **where do schemas live, and who owns them?**

### Current state

- The `@magic-ingredients/no-tickets` npm package owns the v2 push payload zod schemas.
- `no-tickets-service` imports those schemas from npm and uses them to validate `/v1/snapshots` ingest.
- `tiny-brain` imports types from npm to assemble pushes.
- The SDK is the de facto schema authority for the platform's only ingest format.

### What changes

The foundation introduces:
- `event_types` registry table, mirrored from server code on boot.
- `registerEventType({ id, schema, ui, retentionDays, dedupeKey })` API in the server.
- `GET /v1/admin/event-types` introspection endpoint exposing JSON Schema per type.
- Domain PRDs registering ~35 event types across 8+ domains.
- A long-term goal of customer-defined event types (deferred to v2).

Three structural pressures arise:

1. **The catalogue grows weekly.** Every domain PRD adds event types. If the SDK owns them, every new type requires an npm release that the server, tiny-brain, and every integration must install before it works.
2. **Customer-defined types are incompatible with SDK ownership.** A customer cannot add an event type to a package they don't control.
3. **Non-TS consumers exist.** CI runners, Python agents, and shell-script integrations need schemas in a portable format. JSON Schema is the obvious choice; the question is who publishes it and how it's discovered.

### Constraints we are not subject to

The foundation PRD explicitly accepts data loss: "We are explicitly accepting data loss during the transition from the legacy `schema_*` model. There is no migration path; current production data goes away." This means the SDK does **not** need to preserve push v2 or any current schema for backwards compatibility. We can choose the cleanest end-state.

## Decision

**Split schema ownership along the wire/domain boundary. The SDK owns envelopes. The server registry owns the domain catalogue. Discovery is `GET /v1/admin/event-types`.**

### Boundary

The test for which side owns a schema: *if a customer adds their own event type tomorrow, does the SDK need to know about it?* If yes → SDK. If no → server registry. Envelopes pass; domain payloads do not.

**SDK owns** (zod in source, TS types and JSON Schema published as build artifacts):
- `Event` envelope: `{ type, data, source, subject?, occurredAt?, parentEventId?, traceId?, dedupeKey? }`
- `Source` envelope: `{ name, sdkVersion, version?, attributes? }` — required on every event; SDK auto-fills `name` and `sdkVersion` per entry surface; `attributes` is free-form `Record<string, string|number|boolean>` with documented conventions (cookbook, not enforced by schema). Self-reported telemetry — server must not use for authorisation.
- `Subject` envelope (promotion request, reference shape)
- `Interaction` envelope (request and response)
- `Session`, `PushEnvironment`, `Actor` — collapsed into helpers that *construct* `Source` rather than parallel envelope concepts.
- Type-ID grammar — regex `^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*){2}\.v[1-9]\d*$`, with `parseTypeId` / `formatTypeId` helpers.

**Server registry owns** (zod in server code, JSON Schema via `/v1/admin/event-types`):
- All `ai.*`, `engineering.*`, `product.*`, `scm.*`, `support.*`, `team.*`, `flags.*`, future-domain payloads.
- Subject type state schemas (`engineering_service`, `product_feature`, `support_ticket`, …).
- Interaction input schemas.

### Discovery surfaces (in priority order)

1. **`GET /v1/admin/event-types`** — primary at-runtime introspection. Returns JSON Schema per type. Permission-scoped: callers see types they can write. ETag-cacheable.
2. **Auto-generated docs site** — built periodically from the introspection endpoint. Per-type pages with payload examples, dedupe keys, retention, UI hints. The human discovery story.
3. **`@magic-ingredients/no-tickets/types`** — TS types for envelopes only. What every consumer pulls.
4. **Optional codegenned `-types` companion package** — generated from the registry on each server release, pinned to a server commit. TS consumers who want typed `publish([{ type: 'engineering.deploy.completed.v1', data: {...}, ... }])` opt in. Likely ships from `no-tickets-service`, not from this repo.
5. **In-SDK runtime helper** — `client.events.describe(typeId)` fetches the introspection endpoint and caches under `.notickets/registry-cache.json` with an ETag. Powers `nt event list`, `nt event describe`, fuzzy-match suggestions on validation failure, drift notifications on routine commands, and MCP `list_event_types` / `describe_event_type` tools for agents.

### No backwards compatibility for v1

Push v2 will not be preserved. The SDK stops shipping a `Push` schema. AI agents publish `ai.phase.completed.v1`, `ai.review.completed.v1`, etc. directly via `publish([...])`. The server-side push fan-out logic in `domain-ai-telemetry` Feature 1 Task 4 becomes unnecessary. Existing client-PRD features `push-schemas` and `push-cli` are superseded by `publish` and the `nt publish` / `nt event` CLIs in the new client adoption PRD.

### Validation refinements

zod refinements (`.refine()`, custom validators) do not survive JSON Schema export. To avoid silent drift between server-side validation and client-side validation:

- Registered event-type schemas may not use refinements. Cross-field invariants belong in the reducer, not the schema.
- If this proves too restrictive, codegen flags affected types with `// note: server-side validation only` comments, and the docs site marks them. Default is the stricter rule.

## Consequences

### Positive

- **One source of truth for the domain catalogue** — server registry. No "who wins?" question between SDK and server.
- **Server velocity unblocked.** New event types ship without npm releases.
- **Customer-defined types fit naturally.** No corner painted into v2 plans.
- **SDK shrinks and stabilises.** It becomes a wire-format library, not a platform catalogue. Smaller surface, slower change rate, easier to keep stable.
- **Discovery is a real first-class feature**, not an afterthought. Agents can introspect and pick the right type per task — the central UX promise of an event-driven platform.
- **Non-TS consumers served.** JSON Schema from the introspection endpoint is portable.

### Negative

- **Two surfaces to learn.** The mental model is "envelopes here, payloads there." Boundary documentation has to be crisp.
- **Domain TS autocomplete requires opt-in.** Without the codegenned `-types` package, consumers calling `publish([{ type: 'engineering.deploy.completed.v1', data: { ... }, ... }])` get `data: unknown`. Codegen package mitigates but adds a moving piece.
- **Boundary disputes are likely.** Some shapes (e.g. `Subject` reference vs `engineering_service` subject body) will provoke "is this envelope or domain?" debate. Resolution is the customer-defined-types test above; reaffirm whenever it comes up.
- **Refinement ban is real.** Some validations that fit naturally as `.refine()` have to move to reducers. Slight cost in code locality.

### Neutral

- Existing `src/core/types.ts` and `src/core/schemas.ts` shrink dramatically — only envelope-level types survive. Push-related work in completed features (`push-schemas`, `push-cli`, `push-token-cli`) is superseded; tracked but not preserved as live code.

## Alternatives considered

### Alternative A: Server is sole truth, all clients codegen everything

The Stripe/AWS model. Server registers, JSON Schema is the wire spec, every consumer codegens its types from a published schema dump.

Rejected because:
- Forces every TS consumer through codegen even for envelope-level use.
- Requires investment in a polyglot SDK story (codegen for TS, Python, Go, …) that the project is too small to justify today.
- Achieves no better outcome for non-TS consumers than the chosen split — JSON Schema is JSON Schema either way.
- The current decision can evolve toward this in v2 without rework: the SDK boundary stays the same; what changes is whether the codegen package becomes mandatory.

### Alternative B: SDK is sole truth, server imports

The current model, extended. The npm package owns every domain payload schema; server imports.

Rejected because:
- Couples server release velocity to npm publish cadence.
- Cannot accommodate customer-defined event types ever.
- Domain catalogue (35+ types and growing) ends up in a "client" SDK, which is a category mismatch — the SDK becomes the platform.

### Alternative C: Dual-write

Both sides ship schemas, with a CI guard diffing them.

Rejected because:
- Two sources of truth, not one. The diff guard is a tax that becomes the actual source of truth in practice.
- Drift is a question of when, not if.

## Implementation notes

- `client.events.describe()` should serve cached responses synchronously and refresh asynchronously. Network failures must not break `nt event list`.
- The cache lives at `.notickets/registry-cache.json` with the schema `{ etag, fetchedAt, types: EventTypeSpec[] }`.
- Cache age over a configurable threshold (default 14 days) without a successful refresh prints a stale-cache warning, never an error.
- Permission scoping on `/v1/admin/event-types` is non-negotiable — the listing must reflect what the *caller* can write, not the global catalogue, or `nt event list` becomes a list of half-403s.
- The codegenned `-types` package is opt-in and out of scope for the foundation rollout. Track separately.

## Related work

- **`event-repository-foundation` PRD** (`no-tickets-service`) — defines the registry, ingest, introspection endpoint.
- **`domain-*` PRDs** (`no-tickets-service`) — define the event-type catalogue this ADR governs.
- **`client-event-repository-adoption` PRD** (this repo, to be drafted) — implements the SDK side of this decision.
