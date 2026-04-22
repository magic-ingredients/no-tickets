---
id: push-schemas
prd_id: no-tickets-client
number: 3
title: Push Schemas
status: completed
created: 2026-04-22
updated: 2026-04-22
---

# Feature: Push Schemas

## Description

v2 Push payload types and Zod runtime schemas. The schemas are the single source of truth for what data the system accepts. They're published via the npm package and imported by:
- The client (validates before sending)
- The server (validates on ingest)
- tiny-brain (type safety when assembling payloads)
- Any TypeScript consumer

### Push Payload Structure

Generic core envelope with typed extension schemas:
- **Core**: Push (envelope), Session (who pushed), PushEnvironment (where from)
- **project**: Entity hierarchy (epics, features, tasks) — powers the product board
- **dev**: Engineering telemetry (phases, commits, reviews) — powers the engineering board
- **pm**: PM workflow (acceptance, priority, labels, releases) — powers PM views
- **quality**: Quality scores (score, grade, source, categories) — powers quality trends
- **custom**: Escape hatch for user-defined data

### Session Auto-Enrichment

detectAgent() auto-populates session from environment:
- agent: from CLAUDE_SESSION_ID, CURSOR_SESSION_ID, WINDSURF_SESSION_ID
- vendor: derived from agent (anthropic, cursor, codeium)
- environment.os: process.platform
- environment.runtime: process.version
- environment.ci: process.env.CI
- environment.ciProvider: GITHUB_ACTIONS, GITLAB_CI, CIRCLECI, etc.

### Schema Ownership

Schemas live in this package because it's the public npm package that everyone depends on. The update flow:
1. Schema change in client (types.ts + schemas.ts)
2. Publish new client version
3. Server updates dependency, gets new validation
4. tiny-brain updates dependency, gets new types

Schemas are additive — new optional fields and new schema keys never break existing consumers.

## Acceptance Criteria

- [ ] TypeScript types for all Push v2 interfaces exported from ./types
- [ ] Zod schemas for all Push v2 types exported from ./schemas
- [ ] detectAgent() returns v2 Session with auto-enriched environment
- [ ] Schemas validate typed fields, reject malformed data
- [ ] Schemas allow unknown fields in meta (forward-compatible)
- [ ] Schemas allow partial payloads (all extension schemas optional)

## Tasks

### 1. Define v2 Push types (core + all extension schemas)
status: completed
commitSha: 1a6ab6f

Push, Session, PushEnvironment, WorkSchema, WorkEntity, EngineeringSchema, EngineeringTask, EngineeringReview, ProductSchema, ProductUpdate, CodeQualitySchema. All fields readonly. Union types for enums. Renamed per ADR-0004 (schema-domain-rename fix).

**Files modified:**
- `src/core/types.ts`

### 2. Expand detectAgent() for session auto-enrichment
status: completed
commitSha: ec6dc51

Returns v2 Session. Auto-detects vendor (anthropic, cursor, codeium), environment (os, runtime, ci, ciProvider for 6 CI providers).

**Files modified:**
- `src/agent-detect.ts`

### 3. Add Zod runtime schemas for Push v2
status: completed
commitSha: 4bbf22b

Zod schemas matching all v2 types. Used for client-side validation before push and importable by server for ingest validation.

**Files modified:**
- `src/core/schemas.ts`

### 4. Verify sub-path exports
status: completed
commitSha: 4bbf22b

Verified: `./types` exports all 16 v2 type definitions, `./schemas` exports all 24 Zod schemas (12 v1 + 12 v2). No changes needed — existing package.json exports and file structure already work.

**Files modified:**
- None (verification only)

## Dependencies

None — pure types and validation.

## Testing Strategy

### Unit Tests
- Zod schemas accept valid Push payloads (full and partial)
- Zod schemas reject malformed data (wrong types, invalid enums)
- Zod schemas allow partial payloads (missing optional schemas)
- Zod schemas preserve meta fields (passthrough, not validated)
- detectAgent auto-enriches all environment fields
- detectAgent derives vendor from agent name
