---
id: mcp-discovery-tools
prd_id: client-event-repository-adoption
number: 5
title: MCP Discovery Tools
status: not_started
created: 2026-04-27
updated: 2026-05-06
---

# Feature: MCP Discovery Tools

## Description

Expose the registry and the publish/interact/promote primitives as MCP tools so agents can discover and emit events without curated prompts. This is the load-bearing UX promise of the platform: AI agents are the canonical emitters, and discovery has to fit into the agent's tool surface, not a developer's terminal.

Five tools. The publish tool is **singular** (`publish_event`) — one event per call. Agent reasoning loops naturally produce one event per phase, so per-event publish gives per-event errors and per-event recovery. The array shape lives at the wire and SDK layers, not in agent tool calls. No `publish_events` (plural) tool ships in v1; add only when a real agent batch use case is demonstrated.

### Tools

```
list_event_types(domain?: string, deprecated?: boolean)
  → { types: { id, domain, entity, action, version, summary }[] }

describe_event_type(id: string)
  → { id, schema, example, dedupe_strategy, retention_days, ui_hints, deprecated_at }
  // `example` is a synthesised payload from the JSON Schema (shared example-synth lib
  // from Feature 4 Task 2). Lowers the "first event in three calls" bar — agents don't
  // have to interpret raw JSON Schema to construct their first publish.

publish_event(type: string, data: object,
              subject?: { type, id },
              occurred_at?: string,
              parent_event_id?: string,
              trace_id?: string,
              dedupe_key?: string)
  → { id, deduped }
  // Singular: one event per call. `source` is filled by the MCP server, not the agent
  // (agents must not be trusted to self-report runtime). The MCP server passes
  // { name: 'mcp', sdkVersion, attributes: { client, clientVersion } } where client
  // is detected from the MCP transport when possible.

run_interaction(id: string, input: object, subject?: { type, id })
  → { events: { id, type }[] }

create_subject(type: string, external_id: string, display_name: string,
               metadata?: object)
  → { type, id }
```

### Tool description discipline

MCP tool descriptions get loaded into agent context every turn. They need to be useful but not bloated:

- Each tool description ≤ 60 words.
- `list_event_types` description names the type-id grammar so agents understand the namespace.
- `describe_event_type` description tells agents to call this before `publish_event` if they don't already know the schema; mentions the `example` field as a starting point.
- `publish_event` description explicitly mentions calling `describe_event_type` first; the validation error from server-side rejection is also acceptable but slower.

The goal: an agent that has never seen the platform should be able to land its first event after at most three tool calls (`list` → `describe` → `publish_event`).

### Source filling for MCP

The MCP server constructs `source` server-side per call rather than accepting it as a tool argument. Default: `{ name: 'mcp', sdkVersion, attributes: { client, clientVersion? } }` where `client` and `clientVersion` come from MCP transport hints when available (e.g., Claude Code identifies itself in MCP initialization). If transport hints are absent, `attributes.client = 'unknown'`.

Agents cannot override `source` — that would let a malicious agent claim to be a different client. If agent-supplied source attributes are needed in future, add them via a separate, auth-checked path.

### Auth + transport

MCP tools share the same `Client` instance as the CLI (Features 2 + 3). Auth resolution unchanged. Cache shared with CLI usage in the same working directory.

## Acceptance Criteria

- [ ] Five tools registered with the MCP server: `list_event_types`, `describe_event_type`, `publish_event` (singular), `run_interaction`, `create_subject`. No `publish_events` (plural) tool.
- [ ] Tool schemas reject unknown arguments (no agent slop sneaks through).
- [ ] `publish_event` does NOT accept a `source` argument; source is filled server-side from transport hints.
- [ ] Each tool description ≤ 60 words.
- [ ] `describe_event_type` returns a synthesised `example` payload alongside the JSON Schema, using the shared `example-synth` lib from Feature 4.
- [ ] `list_event_types` and `describe_event_type` use the cached registry; refresh fires async.
- [ ] `publish_event` validates `data` against the cached JSON Schema before sending (best-effort; server is authoritative); surfaces server validation errors with field paths if it has to round-trip.
- [ ] Errors returned as structured tool results, not exceptions in the MCP transport.
- [ ] No reference to a `push` tool in the registered tool list.

## Tasks

### 1. Register tools in the MCP server

**Files to modify/create:**
- `src/mcp/server.ts` (or existing MCP entry point)
- `src/mcp/server.test.ts`
- `src/mcp/tools/list-event-types.ts` (new)
- `src/mcp/tools/describe-event-type.ts` (new)
- `src/mcp/tools/publish-event.ts` (new)
- `src/mcp/tools/run-interaction.ts` (new)
- `src/mcp/tools/create-subject.ts` (new)
- `src/mcp/lib/source-from-transport.ts` (new — derives `Source` from MCP transport context)
- `src/mcp/tools/*.test.ts`

**Expected changes:**
- Each tool module exports `{ name, description, inputSchema, handler }`.
- Server registers them on boot.
- `source-from-transport` reads the MCP initialization message (or equivalent) to extract client name/version; returns a default `{ name: 'mcp', sdkVersion, attributes: { client: 'unknown' } }` when no hints are available.
- `describe_event_type` calls the shared `example-synth` lib to attach an `example` field to its response.
- Tests assert registration list (five tools, no `publish_events` plural), description length budget, input schema rejection of extra fields, source-from-transport fallback when transport hints are absent.

### 2. Wire tools to underlying client primitives

**Files to modify/create:**
- Same files as Task 1

**Expected changes:**
- Each handler is a thin wrapper over the corresponding client primitive (`publish`, `subjects.create`, `runInteraction`, `events.list`, `events.describe`).
- `publish_event` wraps `publish([oneEvent])` — the underlying transport is unified (no parallel code path), the singular tool is just a wrapper.
- Source filling happens in the `publish_event` handler before delegating to `publish`: handler builds `Source` from transport context, attaches to event, then calls `publish`.
- No additional logic beyond source filling — this feature is composition, not invention.
- Tests cover the wrapping: each tool delegates correctly with arguments mapped 1:1; `publish_event` arrives at `publish` with a fully-formed source the agent could not have supplied.

### 3. Structured error mapping

**Files to modify/create:**
- `src/mcp/lib/error-mapping.ts` (new)
- `src/mcp/lib/error-mapping.test.ts` (new)

**Expected changes:**
- Convert `UnknownEventTypeError`, `EventValidationError`, `PermissionDeniedError`, `ServerError` to structured MCP tool results with `{ ok: false, error: { code, message, fieldPath? } }`.
- Tests cover each error → expected structured result.

### 4. Remove old push MCP tool

**Files to modify/create:**
- `src/mcp/tools/push.ts` (delete if exists)
- `src/mcp/server.ts` — drop registration

**Expected changes:**
- No `push` tool exposed; agents using it get a clear "tool not found" diagnostic the MCP transport handles.
- Tests assert the tool is gone.

### 5. Smoke test the discovery flow

**Files to modify/create:**
- `src/mcp/__tests__/discovery-flow.test.ts` (new)

**Expected changes:**
- Integration test driving the MCP server with a stubbed underlying client.
- Sequence: `list_event_types` → `describe_event_type` → `publish_event`. Asserts agent-facing outputs at each step are usable for the next: the `id` from `list` flows into `describe`, the `example` from `describe` is a structurally valid `data` for `publish_event`.
- This is the "first event in three calls" guarantee.

## Dependencies

- Feature 2 (HTTP client) — emit/interact/promote primitives.
- Feature 3 (registry introspection) — list/describe primitives.
- Existing MCP server feature in `no-tickets-client` PRD — host process this builds on.

## Testing Strategy

### Unit Tests
- Per-tool argument validation, handler delegation, error mapping.
- Description length budget enforced as a test (catches drift).

### Integration Tests
- Full discovery flow against stubbed transport.
- MCP-specific: tool listing matches the expected set; descriptions render correctly; structured errors deserialise.
