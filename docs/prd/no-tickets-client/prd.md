---
id: no-tickets-client
title: "no-tickets Client — Auth, Push & Schemas"
version: 2.0.0
status: in_progress
created: 2026-04-16
updated: 2026-04-22
author: Andy Richardson
---

# no-tickets Client — Auth, Push & Schemas

## Purpose and Goals

no-tickets is an observability layer for AI agents. Agents send events to a central server; the server stores them; different views render different schemas. The `@magic-ingredients/no-tickets` npm package is the client-side of this system.

The client's responsibilities:
1. **Auth** — authenticate with the no-tickets server (browser OAuth + push tokens)
2. **Push** — send validated events to the server
3. **Schemas** — define and validate the typed event schemas
4. **Distributions** — ship as MCP wrappers for every agent platform

The client does NOT contain PM workflow skills (create epic, break down feature, etc.) — those belong in tiny-brain. The client is the transport layer; tiny-brain (and other orchestrators) are the application layer.

### Design Principles

- **Generic event sink** — the client sends events, it doesn't interpret them
- **Schema as contract** — Zod schemas validate payloads before sending; the server imports the same schemas for ingest validation
- **MCP = thin CLI wrapper** — the MCP server exposes the same operations as the CLI, no separate code path
- **Additive schemas** — new schemas never break existing ones; new optional fields never break existing payloads
- **nt doesn't know its callers** — whether tiny-brain, a CI pipeline, or a ChatGPT agent assembled the payload is irrelevant

### Who Uses What

| Caller | How they push | What they populate |
|--------|--------------|-------------------|
| Claude Code + tiny-brain | CLI hooks (`npx no-tickets push`) | project + dev + quality |
| Claude Code (no orchestrator) | MCP push tool or CLI | project |
| Cursor / Windsurf / Copilot | MCP push tool | project |
| CI pipeline | CLI (`npx no-tickets push --ci`) | quality |
| ChatGPT PM | MCP push tool | project + pm |
| Custom agent framework | CLI --stdin or SDK | any combination |

### Tech Stack

**Language:** TypeScript (strict mode)
**Runtime:** Node.js
**Testing:** Vitest
**Package manager:** pnpm
**Distribution:** npm (`@magic-ingredients/no-tickets`)

## Features and Functionality

### Feature 1: CLI Authentication Flow
**File**: [features/cli-auth-flow.md](features/cli-auth-flow.md)
**Status**: completed
**Description**: Browser-based OAuth flow (`npx no-tickets init`), credential storage (`~/.notickets/credentials`), and auth resolution chain (env var → credentials → prompt).

### Feature 2: Push Token CLI Commands
**File**: [features/push-token-cli.md](features/push-token-cli.md)
**Status**: completed
**Description**: CLI commands for managing push tokens (`token create/list/revoke`) and push command auth via `NO_TICKETS_TOKEN` env var.

### Feature 3: Push Schemas
**File**: [features/push-schemas.md](features/push-schemas.md)
**Status**: completed
**Description**: v2 Push payload types and Zod runtime schemas. Core envelope (Push, Session, PushEnvironment) and extension schemas (project, dev, pm, quality). Session auto-enrichment via detectAgent(). Exported from package sub-paths for server and consumer use.

### Feature 4: Push CLI
**File**: [features/push-cli.md](features/push-cli.md)
**Status**: completed
**Description**: CLI push command that reads `.notickets/`, auto-enriches session, validates against schemas, and sends to server. Supports `--stdin` for orchestrators providing raw payloads. Includes validate command for local format checking.

### Feature 5: End-to-End Tests
**File**: [features/e2e-tests.md](features/e2e-tests.md)
**Status**: completed
**Description**: Comprehensive integration tests for all CLI commands. Tests the I/O wiring that unit tests don't cover — reading `.notickets/` from disk, auth resolution, API calls, stdin piping, error handling, and exit codes. External dependencies mocked at network boundary.

### Feature 6: MCP Server
**File**: [features/mcp-server.md](features/mcp-server.md)
**Status**: completed
**Description**: Thinnest possible MCP wrapper over CLI. Three tools: push (send events), validate (check format), status (auth check). Auto-detected via stdin pipe.

### Feature 7: OSS Launch & npm Publish
**File**: [features/oss-launch.md](features/oss-launch.md)
**Status**: completed
**Description**: CI setup (GitHub Actions), npm publish with provenance, and wiring tiny-brain as a consumer.

### Feature 8: Platform Distribution — MCP Wrappers
**File**: [features/platform-distribution.md](features/platform-distribution.md)
**Status**: completed
**Description**: Platform-specific wrappers packaging the core MCP server for Claude Code, Claude Desktop, Cursor, ChatGPT, Gemini, Copilot, Windsurf, and Continue.dev.

### Feature 9: Migration Script (docs/prd/ → .notickets/)
**File**: [features/migration-script.md](features/migration-script.md)
**Status**: not_started
**Description**: CLI command that converts existing `docs/prd/` repos to `.notickets/` format. Deferred until after launch.

## Release Criteria

### Functional Requirements
- [ ] `npx no-tickets init` authenticates via browser OAuth, saves credentials
- [ ] `npx no-tickets push` reads .notickets/, validates, sends v2 payload
- [ ] `npx no-tickets push --stdin` accepts raw Push payload from orchestrators
- [ ] `npx no-tickets validate` checks .notickets/ format against schemas
- [ ] MCP server exposes push, validate, status tools via stdio
- [ ] Zod schemas importable by server and consumers via npm
- [ ] Works on macOS, Linux, Windows (WSL)

### Technical Requirements
- [ ] All new code has unit tests (TDD)
- [ ] TypeScript strict mode throughout
- [ ] Zero/minimal external dependencies
- [ ] Schemas are the single source of truth (types + Zod, same package)

## Constraints and Dependencies

### Dependencies
- `no-tickets-service` API (v2 push endpoint, auth endpoints, token CRUD)
- Kinde integration (server-side OAuth)
- `@modelcontextprotocol/sdk` (MCP server)
- `zod` (runtime validation)

### Implementation Order

| Order | Feature | Dependency |
|-------|---------|-----------|
| 1 | Feature 1 (CLI Auth) | Server auth endpoint — done |
| 2 | Feature 2 (Push Token CLI) | Server token routes — done |
| 3 | Feature 3 (Push Schemas) | None (pure types + validation) — done |
| 4 | Feature 4 (Push CLI) | Feature 3 (schemas for validation) — done |
| 5 | Feature 5 (E2E Tests) | Features 1-4 (all CLI commands) |
| 6 | Feature 6 (MCP Server) | Feature 4 (wraps CLI functions) — done |
| 7 | Feature 8 (Platform Distribution) | Feature 6 (wrappers reference MCP server) |
| 8 | Feature 7 (OSS Launch) | Features 3-6, 8 (publish with wrappers ready) |
| 9 | Feature 9 (Migration Script) | Feature 7 (npm published) |

### Relationship to tiny-brain

- tiny-brain imports types from `@magic-ingredients/no-tickets/types`
- tiny-brain imports Zod schemas from `@magic-ingredients/no-tickets/schemas` (optional)
- tiny-brain contains PM workflow skills (/plan, /feature, /fix) that create `.notickets/` files on disk
- tiny-brain calls `npx no-tickets push` to send events after file changes
- tiny-brain populates `dev` and `quality` schemas; no-tickets doesn't know about TDD phases, reviews, or quality scores
- Schema updates are client-first: publish new client version → consumers update dependency → types flow through
- Skills and schemas are NOT 1:1 coupled — schemas define what data the system accepts, skills are product decisions about which workflows to automate
