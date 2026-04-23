---
id: push-token-cli
prd_id: no-tickets-client
number: 2
title: Push Token CLI Commands
status: completed
created: 2026-04-16
updated: 2026-04-16
---

# Feature: Push Token CLI Commands

## Description

CLI commands for creating, listing, and revoking push tokens, plus updating the push command to support `NO_TICKETS_TOKEN` env var for CI/CD pipelines.

The server-side push token table, auth middleware, and CRUD routes are already implemented in `no-tickets-service` (Feature 14, Tasks 1-3, 6).

## Acceptance Criteria

- [ ] `npx no-tickets token create --project <id>` creates a push token and displays it once
- [ ] `npx no-tickets token list` shows all tokens (prefix + label + created)
- [ ] `npx no-tickets token revoke <id>` revokes a token
- [ ] `npx no-tickets push` detects `NO_TICKETS_TOKEN` env var and uses it
- [ ] Push token takes precedence when no Kinde session exists
- [ ] Kinde session takes precedence over push token when both exist
- [ ] Clear error message if neither auth method is available

## Tasks

### 1. Add CLI token commands
status: completed
commitSha: cc225fb

CLI commands for creating, listing, and revoking push tokens.

**Files to modify/create:**
- `src/commands/token.ts`

**Expected changes:**
- `npx no-tickets token create --project <id> --label <label>` — calls POST /api/v1/tokens, displays token once
- `npx no-tickets token list` — calls GET /api/v1/tokens, shows prefix + label + created
- `npx no-tickets token revoke <id>` — calls DELETE /api/v1/tokens/:id
- Requires active Kinde session for all token management commands
- Uses auth resolution chain from Feature 1

### 2. Update push command to support token auth
status: completed
commitSha: e9b9517

Modify the push command to detect and use push tokens from environment.

**Files to modify/create:**
- `src/commands/push.ts`
- `src/lib/auth.ts`

**Expected changes:**
- Check for `NO_TICKETS_TOKEN` env var
- If present and no active Kinde session, use token in Authorization header
- If both exist, prefer Kinde session (interactive takes precedence)
- Clear error message if neither auth method is available
- Push token payload omits teamId/projectId (server derives from token)

## Dependencies

- Feature 1 (CLI Auth Flow) — auth resolution chain
- Server-side token CRUD routes (service repo, already implemented)

## Testing Strategy

### Unit Tests
- Token create displays token once and hides on subsequent list
- Token list shows prefix + label only
- Token revoke calls correct endpoint
- Push command detects `NO_TICKETS_TOKEN` env var
- Push command prefers Kinde session when both exist

### Integration Tests
- Full flow: create token → set as NO_TICKETS_TOKEN → push succeeds
- Revoked token → push fails with clear error
