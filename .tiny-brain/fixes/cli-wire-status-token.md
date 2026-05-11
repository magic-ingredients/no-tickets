---
id: cli-wire-status-token
title: "Wire status and token commands into CLI"
status: completed
severity: high
reported: 2026-04-23T08:00:00.000Z
resolved: 2026-04-23T11:25:00.000Z
resolution:
  rootCause: CLI surface was incomplete — commands existed as separate modules but were never dispatched from src/cli.ts
  fix:
    - Wired `status` command with shared describeAuthStatus() helper reused by the MCP status tool
    - Wired `token list | create | revoke` subcommands with allowlist-based value-flag parsing
    - Added NO_TICKETS_HOME override to isolate e2e tests from machine-state credentials
    - Released 2.0.2 via GitHub release (triggers npm publish workflow)
  filesModified:
    - src/cli.ts
    - src/sdk/auth.ts
    - src/sdk/credentials.ts
    - src/mcp/tools/status.ts
    - src/__tests__/status-e2e.test.ts
    - src/__tests__/token-cli-e2e.test.ts
    - src/__tests__/cli.test.ts
    - src/__tests__/mcp-e2e.test.ts
    - src/__tests__/auth-e2e.test.ts
    - src/sdk/__tests__/auth.test.ts
    - src/sdk/__tests__/credentials.test.ts
    - package.json
archived: true
---

# Fix: Wire status and token commands into CLI

## Issue Summary

**Reported:** 2026-04-23
**Severity:** high
**Status:** in_progress

`src/commands/token.ts` and `src/mcp/tools/status.ts` implement auth-status and token CRUD logic with full test coverage, but `src/cli.ts` only dispatches `push` and `validate`. Every other command falls through to "not yet implemented", and `token` is unrecognised entirely.

## Root Cause

Commands were built ahead of CLI wiring as part of the OSS-launch feature set and never plumbed through.

## Tasks

### 1. Wire status command
status: completed
commitSha: ec4a7cb

Dispatch `no-tickets status` to a handler that prints auth state (authenticated, source, tokenType, apiUrl) using the existing `resolveAuth()` helper. Shared `describeAuthStatus()` helper extracted into sdk/auth.ts for reuse by the MCP status tool.

### 2. Wire token command with subcommands
status: completed
commitSha: e07e60f

Add `token` to the known commands. Dispatch subcommands `list`, `create`, `revoke` to `listTokens`/`createToken`/`revokeToken` in `src/commands/token.ts`. Honour `NO_TICKETS_TOKEN` for session auth. Print results as JSON. parseArgs extended with an explicit VALUE_FLAGS allowlist so existing boolean flags aren't regressed.

### 3. Bump to 2.0.2 and release
status: completed
commitSha: pending

Ship as patch.
