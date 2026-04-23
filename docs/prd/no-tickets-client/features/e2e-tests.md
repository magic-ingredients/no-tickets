---
id: e2e-tests
prd_id: no-tickets-client
number: 5
title: End-to-End Tests
status: completed
created: 2026-04-22
updated: 2026-04-22
---

# Feature: End-to-End Tests

## Description

Comprehensive integration tests for the CLI commands and MCP server. Tests the I/O wiring that unit tests don't cover — reading `.notickets/` from disk, auth resolution, API calls, stdin piping, error handling, exit codes, and MCP protocol compliance.

All tests run in-process with Vitest. External dependencies (API server) are mocked at the `fetch` boundary. Filesystem operations use real temporary directories via `mkdtemp`. MCP protocol tested via the SDK's `InMemoryTransport` (no process spawning).

Runs as part of `pnpm test` on CI — no separate step or infrastructure needed.

### What's real vs what's mocked

| Layer | Approach |
|-------|----------|
| Filesystem (.notickets/) | **Real** — temp dirs with real markdown files |
| Environment variables | **Real** — `vi.stubEnv` |
| Auth resolution | **Real** — tests the actual chain |
| Credential files | **Real** — read/write in temp home dir |
| Zod validation | **Real** — validates actual assembled payloads |
| MCP protocol | **Real** — InMemoryTransport, actual Client/Server |
| `fetch` (API calls) | **Mocked** — only boundary we can't test locally |

## Acceptance Criteria

- [ ] Push command tested: read .notickets/ → assemble → validate → send (mocked API)
- [ ] Push --dry-run tested: outputs payload JSON, does not call API
- [ ] Push --stdin tested: reads JSON from stdin, merges session, sends
- [ ] Push error paths tested: missing projectId, invalid files, API 400/500
- [ ] Validate command tested: valid dir passes, invalid dir fails with errors
- [ ] Validate with missing .notickets/ dir: exits cleanly (no crash)
- [ ] Auth resolution tested: env var wins, then credentials, then error message
- [ ] Token commands tested: create/list/revoke lifecycle (mocked API)
- [ ] MCP push tool tested: end-to-end via InMemoryTransport
- [ ] MCP validate tool tested: end-to-end via InMemoryTransport
- [ ] MCP status tool tested: end-to-end via InMemoryTransport
- [ ] All tests use temporary directories — no fixtures, no cleanup issues

## Tasks

### 1. Push command integration tests
status: completed
commitSha: 128fe08

8 tests: successful push, auth header, API response, --dry-run, missing projectId, API errors, session enrichment, empty dir.

**Files created:**
- `src/__tests__/push-e2e.test.ts`

### 2. Validate command integration tests
status: completed
commitSha: 14569eb

4 tests: valid dir passes, invalid dir fails with errors, missing dir graceful, orphan detection.

**Files created:**
- `src/__tests__/validate-e2e.test.ts`

### 3. Auth resolution integration tests
status: completed
commitSha: 14569eb

6 tests: env var resolution, session token type, no-auth error, credential round-trip, expiry, clear.

**Files created:**
- `src/__tests__/auth-e2e.test.ts`

### 4. Token command integration tests
status: completed
commitSha: 14569eb

5 tests: create/list/revoke with mocked API, error handling, Bearer auth.

**Files created:**
- `src/__tests__/token-e2e.test.ts`

### 5. MCP protocol integration tests
status: completed
commitSha: 14569eb

8 tests: InMemoryTransport, tool listing, push/validate/status tools, invalid JSON, invalid schema, auth state.

**Files created:**
- `src/__tests__/mcp-e2e.test.ts`

## Dependencies

- Features 1-4, 6 (all CLI commands and MCP server must be implemented)

## Testing Strategy

### Approach
- Real temp directories via `mkdtemp` — no mocking `node:fs/promises`
- Mock `fetch` globally via `vi.stubGlobal('fetch', ...)` for API calls
- `vi.stubEnv` for environment variables
- `vi.spyOn(console, 'log')` / `vi.spyOn(console, 'error')` for output capture
- `process.exitCode` for error exit assertions
- MCP SDK `Client` + `InMemoryTransport` for protocol testing
- File naming: `*.e2e.test.ts` for filtering (`pnpm test -- --grep "e2e"`)
