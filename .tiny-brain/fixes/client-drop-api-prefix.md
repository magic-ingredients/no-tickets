---
id: client-drop-api-prefix
title: "Drop /api from client request paths"
status: completed
severity: high
reported: 2026-04-23T12:30:00.000Z
resolved: 2026-04-23T12:55:00.000Z
resolution:
  rootCause: Client baked in the CF Pages edge `/api` prefix; the API itself mounts routes at /v1/*.
  fix:
    - Flat rename /api/v1/ → /v1/ across 8 files
    - Released 2.0.3 via GitHub release
  filesModified:
    - src/sdk/api-client.ts
    - src/sdk/__tests__/api-client.test.ts
    - src/commands/token.ts
    - src/commands/__tests__/token.test.ts
    - src/__tests__/token-cli-e2e.test.ts
    - src/__tests__/token-e2e.test.ts
    - src/__tests__/push-e2e.test.ts
    - src/__tests__/mcp-e2e.test.ts
    - package.json
archived: true
---

# Fix: Drop /api from client request paths

## Issue Summary

The server at `api.no-tickets.com` mounts routes at `/v1/...`. The `/api/v1/...` prefix is a Cloudflare Pages edge convention that only applies when the SPA proxies to the API. The CLI/SDK talks to the API directly and should use bare `/v1/...` paths.

Verified with probes against prod and staging:
- `api.no-tickets.com/v1/tokens` → 401 (route exists, needs auth) ✓
- `api.no-tickets.com/api/v1/tokens` → 404 (no such route)

## Root Cause

Client hardcoded `/api/v1/...` paths, baking in a proxy-layer artefact. The API itself is clean.

## Tasks

### 1. Strip /api from client request paths
status: completed
commitSha: pending

Flat rename `/api/v1/` → `/v1/` across 8 files via sed.

### 2. Bump to 2.0.3 and release
status: completed
commitSha: pending

Ship as patch.
