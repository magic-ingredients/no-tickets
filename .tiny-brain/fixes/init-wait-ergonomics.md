---
id: init-wait-ergonomics
title: "Make `init` wait less hostile when the server doesn't redirect"
status: completed
severity: minor
reported: 2026-04-24T08:30:00.000Z
resolved: 2026-04-24T08:40:00.000Z
archived: true
---

# Fix: `init` wait ergonomics

## Issue Summary

`no-tickets init` opens the browser and then waits silently up to 120s for the loopback callback. When the server's `/auth/cli` route doesn't redirect back (current bug for already-authenticated browser sessions), the CLI just hangs with no signal. Ctrl-C works but isn't obvious.

## Tasks

### 1. Timeout knob + wait indicator + SIGINT handler
status: completed
commitSha: 3f1d221

`NO_TICKETS_AUTH_TIMEOUT_MS` env + `--timeout <ms>` flag (flag wins on conflict; invalid flag hard-fails). Periodic 10s "Still waiting…" hint, .unref()'d, skipped when timeoutMs < interval. SIGINT closes the auth server and exits 130; race-guarded by a `completed` flag so a SIGINT after success is a no-op.

### 2. --env staging|production preset (resolves both API + auth URLs)
status: completed
commitSha: 0012331

Pivoted from baked-in env presets to: A) echo resolved URLs on init + add authUrl to status, B) pair-validation fails fast on half-set env vars, C) `--profile <name>` loads URLs from `~/.notickets/config.json` (never committed). Resolution order: --profile > env vars > production defaults. Plumbed through init / push / token / status. Shadow-warning when --profile shadows env vars. Help text updated.

### 3. Ship under v2.0.5
status: completed
commitSha: pending

2.0.5 is unpublished — bundle this into the same release rather than cutting 2.0.6.
