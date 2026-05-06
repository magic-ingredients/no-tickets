---
id: mcp-discovery-tools
prd_id: client-event-repository-adoption
number: 5
title: MCP Discovery Tools
status: not_started
created: 2026-04-27
updated: 2026-04-27
---

# Feature: MCP Discovery Tools

## Description

Expose the registry and the emit/interact/promote primitives as MCP tools so agents can discover and emit events without curated prompts. This is the load-bearing UX promise of the platform: AI agents are the canonical emitters, and discovery has to fit into the agent's tool surface, not a developer's terminal.

Five tools, mirroring the CLI verbs but shaped for agent consumption (concise descriptions, structured outputs, no human-decoration concerns).

### Tools

```
list_event_types(domain?: string, deprecated?: boolean)
  → { types: { id, domain, entity, action, version, summary }[] }

describe_event_type(id: string)
  → { id, schema, dedupe_strategy, retention_days, ui_hints, deprecated_at }

publish_events(type: string, data: object, subject?: { type, id }, source?: string,
           occurred_at?: string, parent_event_id?: string, trace_id?: string,
           dedupe_key?: string)
  → { id, deduped }

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
- `describe_event_type` description tells agents to call this before `publish_events` if they don't already know the schema.
- `publish_events` description explicitly mentions calling `describe_event_type` first; the validation error from server-side rejection is also acceptable but slower.

The goal: an agent that has never seen the platform should be able to land its first event after at most three tool calls (`list` → `describe` → `emit`).

### Auth + transport

MCP tools share the same `Client` instance as the CLI (Features 2 + 3). Auth resolution unchanged. Cache shared with CLI usage in the same working directory.

## Acceptance Criteria

- [ ] All five tools registered with the MCP server.
- [ ] Tool schemas reject unknown arguments (no agent slop sneaks through).
- [ ] Each tool description ≤ 60 words.
- [ ] `list_event_types` and `describe_event_type` use the cached registry; refresh fires async.
- [ ] `publish_events` validates `data` against the cached JSON Schema before sending; surfaces server validation errors with field paths if it has to round-trip.
- [ ] Errors returned as structured tool results, not exceptions in the MCP transport.
- [ ] No reference to a `push` tool in the registered tool list.

## Tasks

### 1. Register tools in the MCP server

**Files to modify/create:**
- `src/mcp/server.ts` (or existing MCP entry point)
- `src/mcp/server.test.ts`
- `src/mcp/tools/list-event-types.ts` (new)
- `src/mcp/tools/describe-event-type.ts` (new)
- `src/mcp/tools/emit-event.ts` (new)
- `src/mcp/tools/run-interaction.ts` (new)
- `src/mcp/tools/create-subject.ts` (new)
- `src/mcp/tools/*.test.ts`

**Expected changes:**
- Each tool module exports `{ name, description, inputSchema, handler }`.
- Server registers them on boot.
- Tests assert registration list, description length budget, input schema rejection of extra fields.

### 2. Wire tools to underlying client primitives

**Files to modify/create:**
- Same files as Task 1

**Expected changes:**
- Each handler is a thin wrapper over the corresponding client primitive (`publish`, `subjects.create`, `runInteraction`, `events.list`, `events.describe`).
- No additional logic — this feature is composition, not invention.
- Tests cover the wrapping: each tool delegates correctly with arguments mapped 1:1.

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
- Sequence: `list_event_types` → `describe_event_type` → `publish_events`. Asserts agent-facing outputs at each step are usable for the next.
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
