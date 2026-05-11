---
id: wire-init-command
title: "Wire init command to browser OAuth flow"
status: completed
severity: high
reported: 2026-04-23T17:20:00.000Z
resolved: 2026-04-24T08:10:00.000Z
archived: true
---

# Fix: Wire init command to browser OAuth flow

## Issue Summary

`npx no-tickets init` reports "Command 'init' is not yet implemented.", blocking real authentication. The OAuth browser flow is already built in `src/commands/init-auth.ts` (`resolveInitAuth`) — the CLI just never dispatched to it. The PRD (cli-auth-flow.md) defines `init` as the browser OAuth command.

## Root Cause

CLI `init` case in src/cli.ts is a stub. Consistent with the status/token gap resolved in 2.0.2, auth flow was built before dispatch wiring.

## Tasks

### 1. Wire init command to resolveInitAuth
status: completed
commitSha: 989c37a

Dispatch `no-tickets init` to a handler that:
- Short-circuits if existing credentials are present (prints email + skip message)
- Otherwise opens browser to `NO_TICKETS_AUTH_URL` (default `https://app.no-tickets.com/auth/cli`) with a `callback_port` query param
- Waits for the local auth server to receive the token
- Saves credentials, prints success

Cross-platform browser opener (macOS `open`, Linux `xdg-open`, Windows `start`). If the opener fails, print the URL so the user can paste it manually.

### 2. Bump to 2.0.4 and release
status: completed
commitSha: 165d9cb

Ship as patch.

### 3. Align CLI with hardened /auth/cli protocol (port + code + email)
status: completed
commitSha: 301ff6b

CLI now talks the new contract: app.no-tickets.com/api/auth/cli, port=PORT, code=NONCE, callback returns {token, email}; auth-server enforces timingSafeEqual state match, GET-only /callback, race-fix on close, raw-query parsing to preserve `+` in emails.

### 4. Bump to 2.0.5 and release
status: completed
commitSha: pending

Ship as patch.
