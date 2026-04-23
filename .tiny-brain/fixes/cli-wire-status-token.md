---
id: cli-wire-status-token
title: "Wire status and token commands into CLI"
status: in_progress
severity: high
reported: 2026-04-23T08:00:00.000Z
resolved: null
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
status: not_started

Add `token` to the known commands. Dispatch subcommands `list`, `create`, `revoke` to `listTokens`/`createToken`/`revokeToken` in `src/commands/token.ts`. Honour `NO_TICKETS_TOKEN` for session auth. Print results as JSON (list) or human-readable messages (create/revoke).

### 3. Bump to 2.0.2 and release
status: not_started

Ship as patch.
