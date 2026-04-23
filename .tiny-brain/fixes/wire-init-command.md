---
id: wire-init-command
title: "Wire init command to browser OAuth flow"
status: completed
severity: high
reported: 2026-04-23T17:20:00.000Z
resolved: 2026-04-23T17:40:00.000Z
resolution:
  rootCause: CLI `init` case was stubbed; OAuth logic existed in init-auth.ts but was never dispatched.
  fix:
    - handleInit dispatches to resolveInitAuth, prints URL for manual paste, opens browser
    - Cross-platform spawn-based opener with on('spawn') success signal (fixes silently-swallowed spawn errors)
    - openBrowser plumbed through runCli(argv, deps?) DI (no globalThis hook)
    - Placeholder email no longer surfaced on fresh auth
    - Released 2.0.4
  filesModified:
    - src/cli.ts
    - src/__tests__/init-cli-e2e.test.ts
    - src/__tests__/cli.test.ts
    - package.json
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
commitSha: pending

Ship as patch.
