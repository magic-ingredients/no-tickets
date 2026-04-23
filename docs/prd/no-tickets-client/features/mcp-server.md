---
id: mcp-server
prd_id: no-tickets-client
number: 6
title: MCP Server
status: completed
created: 2026-04-22
updated: 2026-04-22
---

# Feature: MCP Server

## Description

Thinnest possible MCP wrapper over the CLI. The MCP server exposes the same operations as the CLI commands, calling the same core functions. Auto-detected via stdin pipe (see ADR-0003).

Three tools:
- **push** — send a v2 Push payload to the server
- **validate** — check .notickets/ format against schemas
- **status** — check auth and connection

The MCP server is NOT a separate code path. It calls the same functions that the CLI calls — just with structured MCP input instead of argv.

### Why thin?

The MCP server's only job is to translate between MCP protocol and the core push/validate/status functions. All business logic lives in the core functions (Feature 4). This means:
- No MCP-specific bugs — if the CLI works, the MCP works
- No duplicate code paths
- Easy to test (mock the core functions, test the MCP wrapper)

## Acceptance Criteria

- [ ] MCP server auto-detected via stdin pipe (no --mcp flag)
- [ ] `push` tool accepts a Push payload, validates via schemas, auto-enriches session, sends
- [ ] `validate` tool checks .notickets/ format, returns validation errors
- [ ] `status` tool returns auth and connection state
- [ ] All tools return structured JSON responses via MCP protocol
- [ ] Authenticated via NO_TICKETS_TOKEN env var
- [ ] Uses stdio transport

## Tasks

### 1. Rewrite MCP server with 3 tools
status: completed
commitSha: 2f3b09b

Replaced 10-tool registry with push, validate, status. Each tool has Zod input schema, description, and handler wired to CLI core functions.

**Files modified:**
- `src/mcp/create-server.ts`
- `src/mcp/tools/push.ts` (new)
- `src/mcp/tools/validate.ts` (new)
- `src/mcp/tools/status.ts` (new)
- `src/mcp/tools/types.ts` (updated)
- Deleted: `src/mcp/tools/board-feed.ts`, `src/mcp/tools/creation.ts`

### 2. Wire MCP tools to CLI core functions
status: completed
commitSha: 2f3b09b

Handlers implemented in the same commit as task 1 (tightly coupled). push → Zod validate + mergeSession + API push. validate → readNoTicketsDir + validateFiles. status → resolveAuth.

**Files modified:**
- Same as task 1

## Dependencies

- Feature 3 (Push Schemas) — Zod schemas for push tool input validation
- Feature 4 (Push CLI) — core push/validate/status functions to wrap

## Testing Strategy

### Unit Tests
- push tool validates input and calls core push function
- validate tool calls core validate function, returns errors
- status tool calls core status function, returns auth state
- Invalid MCP input returns clear error response
- MCP server registers exactly 3 tools
