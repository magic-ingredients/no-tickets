---
id: push-cli
prd_id: no-tickets-client
number: 4
title: Push CLI
status: completed
created: 2026-04-22
updated: 2026-04-22
---

# Feature: Push CLI

## Description

The CLI push command — the core action of the client. Reads `.notickets/` files, auto-enriches session, validates against Zod schemas, and sends to the server via POST /v1/push.

Two modes:
- `npx no-tickets push` — reads .notickets/ directory, assembles Push payload from files
- `npx no-tickets push --stdin` — accepts raw Push payload JSON from orchestrators (e.g., tiny-brain piping dev/quality data)

Both modes auto-enrich session and validate against schemas before sending.

### CLI Surface

```
npx no-tickets push              # read .notickets/, assemble, validate, send
npx no-tickets push --stdin      # accept raw Push JSON, merge session, validate, send
npx no-tickets push --dry-run    # preview payload without sending
npx no-tickets validate          # check .notickets/ format locally (no network)
npx no-tickets status            # auth and connection check
```

## Acceptance Criteria

- [ ] `npx no-tickets push` reads .notickets/, assembles v2 Push payload, sends to server
- [ ] `npx no-tickets push --stdin` accepts raw Push JSON, merges auto-enriched session, validates, sends
- [ ] `npx no-tickets push --dry-run` prints assembled payload without sending
- [ ] `npx no-tickets validate` validates .notickets/ format locally, returns errors
- [ ] Auto-enriches session on every push (agent, vendor, environment)
- [ ] Validates against Zod schemas before sending — clear error messages on failure
- [ ] API client sends to POST /v1/push with auth header
- [ ] Exit code 0 on success, 1 on validation/send failure

## Tasks

### 1. Add push() to API client
status: completed
commitSha: 59fe202

Single method that sends a validated Push payload to POST /v1/push. CRUD methods retained for now — cleanup deferred to MCP server rewrite.

**Files modified:**
- `src/sdk/api-client.ts`

### 2. Build .notickets/ reader
status: completed
commitSha: 922724e

Added toWorkEntities() to state.ts — converts ParseResult into flat WorkEntity array for v2 Push work schema. Reuses existing parser. Entities use parentId hierarchy. (Originally toProjectEntities/ProjectEntity; renamed per schema-domain-rename fix / ADR-0004.)

**Files modified:**
- `src/core/state.ts`

### 3. Implement CLI push command
status: completed
commitSha: 3114518

Assemble Push payload: read .notickets/ for project entities, auto-enrich session via detectAgent(), validate against Zod schemas, send via API client. Supports --dry-run and --stdin flags. Pure assembly logic in commands/push.ts, I/O wiring in cli.ts.

**Files modified:**
- `src/cli.ts`
- `src/commands/push.ts`

### 4. Implement CLI validate command
status: completed
commitSha: c8228cb

validateFiles() chains parseFiles → validate. CLI reads .notickets/, prints errors with file paths and suggestions, exits 1 on failure.

**Files modified:**
- `src/cli.ts`
- `src/commands/validate.ts`

## Dependencies

- Feature 3 (Push Schemas) — Zod schemas for validation
- Feature 1 (CLI Auth) — auth resolution for API calls
- Feature 2 (Push Tokens) — token auth for CI/MCP

## Testing Strategy

### Unit Tests
- .notickets/ reader produces correct WorkEntity array from files
- Push payload assembled correctly (files + session enrichment)
- --stdin merges auto-enriched session into provided payload
- --dry-run prints payload, does not call API
- Validation catches malformed .notickets/ files with clear messages
- API client sends correct payload shape to correct endpoint
