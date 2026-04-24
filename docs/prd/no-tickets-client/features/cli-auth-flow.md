---
id: cli-auth-flow
prd_id: no-tickets-client
number: 1
title: CLI Authentication Flow
status: completed
created: 2026-04-16
updated: 2026-04-17
---

# Feature: CLI Authentication Flow

## Description

Browser-based OAuth flow for `npx no-tickets init`. Same pattern as `gh auth login` — CLI opens browser, user authenticates via Kinde, token returned to localhost callback, saved to `~/.notickets/credentials`.

The server-side auth endpoint (`/auth/cli`) is tracked separately in the service repo PRD (Feature 13, Task 1).

### Auth Model

| Context | Auth method | Stored where |
|---------|------------|-------------|
| Interactive CLI (init, token create) | Browser OAuth → session token | `~/.notickets/credentials` |
| Dev pushing (npx no-tickets push) | Session token from credentials file | `~/.notickets/credentials` |
| CI pipeline (npx no-tickets push) | Push token via `NO_TICKETS_TOKEN` env var | CI secrets |
| MCP server | Push token via `NO_TICKETS_TOKEN` env var | MCP config env block |

### Auth Resolution Order

1. `NO_TICKETS_TOKEN` env var (push token — for CI/MCP)
2. `~/.notickets/credentials` file (session token — for interactive)
3. Neither found → prompt: "Run `npx no-tickets init` to authenticate"

## Acceptance Criteria

- [ ] `npx no-tickets init` opens browser for OAuth if not authenticated
- [ ] Localhost callback server receives token from browser redirect
- [ ] Token saved to `~/.notickets/credentials` with 600 permissions
- [ ] Subsequent CLI commands use saved token (no re-auth)
- [ ] `NO_TICKETS_TOKEN` env var takes precedence over credentials file
- [ ] Clear error message if both auth methods fail
- [ ] Works on macOS, Linux, Windows (WSL)

## Tasks

### 1. Build CLI localhost callback server
status: completed
commitSha: 9f6f7be

Receives the token redirect from the browser OAuth flow.

**Files to modify/create:**
- `src/sdk/auth-server.ts`

**Expected changes:**
- Start HTTP server on random available port
- Accept an `expectedState` (CSRF nonce) at construction; reject any callback whose `state` query param does not match
- Listen for GET `/callback?token=xxx&email=yyy&state=NONCE`
- Extract token + email, shut down server, resolve `callbackPromise` with `{ token, email }`
- Reject callbacks with method other than GET, missing/empty token, missing email, or mismatched state with HTTP 400 — and do not settle the promise (legitimate flow keeps waiting until timeout)
- Timeout after 120 seconds (user didn't complete login)
- Handle port conflicts gracefully

**Wire format the CLI uses to talk to the server:**
- CLI generates `code = randomBytes(16).toString('hex')` (128-bit CSRF nonce)
- Browser opens `https://app.no-tickets.com/api/auth/cli?port=PORT&code=NONCE`
- Server's localhost redirect: `http://127.0.0.1:PORT/callback?token=…&email=…&state=NONCE` where `state` echoes the `code` the CLI sent

### 2. Build credential storage
status: completed
commitSha: 4aecdb2

Read/write/refresh credentials file.

**Files to modify/create:**
- `src/sdk/credentials.ts`

**Expected changes:**
- `saveCredentials(token, email, expiresAt)` → write to `~/.notickets/credentials`
- `loadCredentials()` → read file, check expiry, return token or null
- `clearCredentials()` → delete file
- Set file permissions to 600 on write (POSIX only, skip on Windows)
- Create `~/.notickets/` directory if it doesn't exist
- Never log or print the full token

### 3. Build auth resolution chain
status: completed
commitSha: 0d2e6a4

Determine which auth to use for any CLI command.

**Files to modify/create:**
- `src/sdk/auth.ts`

**Expected changes:**
- `resolveAuth()`: check env var → check credentials file → return token or throw
- Used by push, status, token commands
- Push token (`nt_push_*` prefix) and session token (`nt_session_*` prefix) are distinguishable
- Server validates both but with different permissions (push token = push only, session = full access)

### 4. Wire auth into init command
status: completed
commitSha: 400255a

Connect the OAuth flow to the init UX.

**Files to modify/create:**
- `src/cli/commands/init.ts`

**Expected changes:**
- Check `loadCredentials()` first
- If valid: skip auth, proceed to team/project selection
- If expired or missing: run browser OAuth flow
- After auth: list teams → list projects → write .notickets.yml
- Handle: browser fails to open (print URL for manual copy)
- Handle: user cancels (Ctrl+C during wait)

## Dependencies

- Server-side `/auth/cli` endpoint (service repo, Feature 13 Task 1)
- Server-side session token table (service repo, Feature 13 Task 6)

## Testing Strategy

### Unit Tests
- Credential storage: write, read, expiry check, clear
- Auth resolution: env var wins, then credentials, then error
- Token prefix detection (push vs session)
- Localhost server starts, receives callback, shuts down

### Integration Tests
- Full OAuth flow: init → browser → callback → credentials saved → push works
- Expired token → init prompts re-auth
- CI mode: NO_TICKETS_TOKEN env var → push succeeds without credentials file
