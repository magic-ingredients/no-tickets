---
id: init-wait-ergonomics
title: "Make `init` wait less hostile when the server doesn't redirect"
status: in_progress
severity: minor
reported: 2026-04-24T08:30:00.000Z
resolved: null
---

# Fix: `init` wait ergonomics

## Issue Summary

`no-tickets init` opens the browser and then waits silently up to 120s for the loopback callback. When the server's `/auth/cli` route doesn't redirect back (current bug for already-authenticated browser sessions), the CLI just hangs with no signal. Ctrl-C works but isn't obvious.

## Tasks

### 1. Timeout knob + wait indicator + SIGINT handler
status: not_started

- New `NO_TICKETS_AUTH_TIMEOUT_MS` env var (defaults to 120_000) so dev/test runs can fast-fail.
- Periodic "Still waiting for browser callback (Xs / Ys)…" line every 10s so the hang isn't silent.
- SIGINT handler: closes the local auth server, prints `Cancelled.`, exits 130.

### 2. Ship under v2.0.5
status: not_started

2.0.5 is unpublished — bundle this into the same release rather than cutting 2.0.6.
