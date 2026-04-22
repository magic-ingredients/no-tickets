---
id: schema-domain-rename
title: "Rename push payload schemas to domain conventions (ADR-0004)"
status: in_progress
severity: high
reported: 2026-04-22T19:30:00.000Z
resolved: null
---

# Fix: Rename push payload schemas to domain conventions (ADR-0004)

## Issue Summary

**Reported:** 2026-04-22
**Severity:** high
**Status:** not_started

The server (no-tickets-service) has been renamed per ADR-0004 to use domain-aligned schema names. The client still uses the old names. The push endpoint now validates against the new field names, so the client is broken until renamed.

### Renames Required

**Push payload fields:**
- `project` → `work`
- `dev` → `engineering`
- `pm` → `product`
- `quality` → `codeQuality`

**Types:**
- `ProjectSchema` → `WorkSchema`
- `ProjectEntity` → `WorkEntity`
- `ProjectEntityType` → `WorkEntityType`
- `DevSchema` → `EngineeringSchema`
- `DevTask` → `EngineeringTask`
- `DevReview` → `EngineeringReview`
- `DevPhase` → `EngineeringPhase`
- `PMSchema` → `ProductSchema`
- `PMUpdate` → `ProductUpdate`
- `QualitySchema` → `CodeQualitySchema`
- `QualitySource` → `CodeQualitySource`

### Affected Files

- `src/core/types.ts` — type definitions
- `src/core/schemas.ts` — Zod validation schemas
- `src/core/state.ts` — state computation (assembles Push payload)
- `src/core/__tests__/state.test.ts` — state tests
- `src/core/__tests__/push-schemas.test.ts` — schema validation tests
- `src/commands/push.ts` — push CLI command
- `src/commands/__tests__/push.test.ts` — push command tests
- `src/sdk/api-client.ts` — SDK client
- `src/sdk/__tests__/api-client.test.ts` — SDK tests
- `src/mcp/tools/push.ts` — MCP push tool
- `src/mcp/create-server.ts` — MCP server
- `src/__tests__/push-e2e.test.ts` — e2e tests
- `src/__tests__/mcp-e2e.test.ts` — MCP e2e tests
- `src/agent-detect.ts` — agent detection (imports Session type)

## Root Cause Analysis

Server-side schema naming was updated to use domain-aligned names per ADR-0004 (schema table conventions). The client types and push assembly code still use the old generic names (`project`, `dev`, `pm`, `quality`). The server's Zod validation now rejects payloads with the old field names.

## Tasks

### 1. Rename types in core/types.ts
Rename all type definitions and the Push interface fields. No back-compat aliases needed — package v2.0.0 had no external consumers at the time of this rename, so it is shipped as a clean break in 2.0.1.

**Files to modify:**
- `src/core/types.ts`

### 2. Update Zod schemas in core/schemas.ts
Rename schema objects and the pushSchema field names to match the new type names.

**Files to modify:**
- `src/core/schemas.ts`
- `src/core/__tests__/push-schemas.test.ts`

### 3. Update state computation
Update computeState and related functions to assemble the Push payload with new field names.

**Files to modify:**
- `src/core/state.ts`
- `src/core/__tests__/state.test.ts`

### 4. Update push command and SDK
Update CLI push command, SDK client, and MCP tools to use new field names.

**Files to modify:**
- `src/commands/push.ts`
- `src/commands/__tests__/push.test.ts`
- `src/sdk/api-client.ts`
- `src/sdk/__tests__/api-client.test.ts`
- `src/mcp/tools/push.ts`
- `src/mcp/create-server.ts`
- `src/__tests__/push-e2e.test.ts`
- `src/__tests__/mcp-e2e.test.ts`
