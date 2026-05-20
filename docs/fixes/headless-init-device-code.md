---
id: headless-init-device-code
title: "`no-tickets init` doesn't work on headless hosts (no browser, no loopback reachability)"
status: not_started
severity: medium
reported: 2026-05-20T00:00:00.000Z
resolved: null
---

# Fix: `no-tickets init` headless / no-browser fallback

## Issue Summary

`no-tickets init` auths via a loopback OAuth callback: the CLI binds a
local listener on `127.0.0.1:<random>`, opens the user's browser at the
server's auth URL with `?port=<port>&code=<nonce>`, and waits for the
server to redirect back to `http://127.0.0.1:<port>/callback?...`. See
`crates/nt-cli/src/commands/init.rs:1-95` + `crates/nt-cli/src/auth_server.rs:53-95`.

This flow assumes (a) a graphical environment that can open a browser,
and (b) loopback reachability between the machine running the CLI and
the machine running the browser. Both assumptions break on common
sandbox / demo / CI / remote-dev setups:

- **Multipass / Docker / cloud VM** — no `xdg-open`, no DISPLAY. `opener`
  crate fails or no-ops. Even if a URL is printed, the browser running
  on the user's workstation can't reach `127.0.0.1:<port>` on the VM
  without an SSH-style port forward.
- **SSH session into a remote dev box** — same problem.
- **GitHub Codespaces / devcontainers** — similar; the editor terminal
  is on the container, browser is on the user's laptop, loopback is
  per-container.
- **Tmux / screen sessions on a server** — no DISPLAY in the session.

Reproducible today by spinning up a Multipass VM (`multipass launch
--name nt-sandbox 24.04; multipass shell nt-sandbox`), installing via
`curl -fsSL https://get.no-tickets.com | sh`, and running `no-tickets
init`. The browser-open call no-ops or errors, the URL printed to
stderr points at the server, the server's redirect target then points
at `http://127.0.0.1:<random>/callback` *on the VM*, which the user's
laptop browser can't reach.

Workaround today: skip `init` entirely, mint a token on a graphical
workstation, paste it into the VM via `no-tickets token add`. That
defeats the whole "fresh install" demo UX — exactly the friction the
sandbox setup is supposed to validate.

## Root Cause

`init` was designed against the workstation-installs-on-workstation
case (a developer on their own laptop). The two assumptions it bakes
in — local browser + loopback reachability — are tight for that case
but exclude every "remote shell" environment, which has become the
common sandbox/demo pattern in 2026.

The OAuth callback design itself isn't wrong (it's the same flow `gh
auth login` uses for its default web path), but it's missing the
companion **device-code** flow (OAuth 2.0 RFC 8628) that every
mature CLI offers as the headless fallback. `gh auth login` has it
(`--web` vs the default device-code prompt); `gcloud auth login` has
`--no-launch-browser`; `aws configure sso` uses device-code; `doctl
auth init` likewise. We're the odd one out.

## Fix Approach

Add a non-browser-callback authentication path. Two viable designs;
pick one (or implement A with B as a transitional first cut):

### Option A — Device-code grant (RFC 8628). Preferred long-term.

Standard OAuth pattern. Requires server-side work in
`magic-ingredients/no-tickets-service` to expose:

- `POST /auth/device/code` — returns `{ device_code, user_code,
  verification_uri, verification_uri_complete, expires_in, interval }`.
  `user_code` is short and human-typeable (8 chars, `WXYZ-ABCD`).
- `GET /auth/device` — verification page that prompts the user for
  the `user_code`, authenticates them via the existing session
  cookie, and binds the device.
- `POST /auth/device/token` — CLI polls this with `device_code` until
  the server returns `{ token, email, expires_at }` or an error
  (`authorization_pending` / `slow_down` / `expired_token` /
  `access_denied`).

CLI flow:

```
$ no-tickets init --device
Visit: https://app.no-tickets.com/auth/device
Enter code: WXYZ-ABCD
Expires in 15 minutes.
Waiting…
✓ Authenticated as andy@magic-ingredients.com.
```

Pros: industry-standard, no token-in-clipboard hop, polling cadence
is server-controlled, works on any device with a browser.

Cons: server-side work (cross-repo). Polling is mildly less elegant
than callback, but well-understood.

### Option B — Manual token paste. Achievable faster.

CLI prints a verification URL; user opens it on any browser-equipped
device, authenticates, and is shown a freshly-minted CLI token to
copy. CLI reads from stdin and saves.

Server-side: one new page (`/cli-token` or similar) that mints a token
on visit (under existing session auth) and renders it once with a copy
button. Single endpoint, no polling protocol.

CLI flow:

```
$ no-tickets init --paste
Visit https://app.no-tickets.com/cli-token in any browser, then paste
the token shown:
> nt_session_…
✓ Authenticated as andy@magic-ingredients.com.
```

Pros: smallest server-side surface, fastest to ship, works the same
on every host.

Cons: token transits the user's clipboard (mildly worse than A's
device-code-bound exchange), more user-error-prone (paste fails,
truncation).

### Recommendation

Ship A. It's the right long-term shape and matches every other CLI
in the category. If server-side device-code work needs to wait on
no-tickets-service capacity, ship B as a transitional first cut and
re-evaluate before declaring `init` done.

### Auto-detection (orthogonal to A vs B)

Detect headless environments and prefer the fallback automatically:

- Linux: neither `$DISPLAY` nor `$WAYLAND_DISPLAY` set
- WSL: `$WSL_DISTRO_NAME` set (browser opens in Windows host, but
  loopback callback still doesn't reach into WSL by default — same
  problem class)
- macOS: always has GUI capability; skip detection
- `$SSH_TTY` set: probably remote shell; suggest the fallback even
  if `$DISPLAY` is forwarded

Detection should suggest, not force — a `--web` flag forces the
existing callback flow when the user knows the tunneling will work
(e.g., devcontainer port forwarding configured).

## Test Plan

### 🔒 Regression Tests (must pass unchanged)
| File | Cases | Status |
|------|-------|--------|
| crates/nt-cli/src/commands/init.rs (inline tests) | `build_callback_url_*`, `generate_nonce_*`, `hex_encode_*`, `expires_at_iso8601_*` | ❌ |
| crates/nt-cli/src/auth_server.rs (inline tests) | callback parsing, state validation, request-line parsing | ❌ |
| crates/nt-cli/tests/init.rs (integration tests, if present) | existing happy-path callback flow | ❌ |

### 🆕 New Tests
| File | Case | Status |
|------|------|--------|
| crates/nt-cli/src/commands/init_device.rs (new) | requests device_code, prints user_code + verification_uri | ❌ |
| crates/nt-cli/src/commands/init_device.rs | polls token endpoint, handles `authorization_pending`, `slow_down`, `expired_token`, `access_denied` | ❌ |
| crates/nt-cli/src/commands/init_device.rs | save_credentials called with server-returned token + email + expires_at | ❌ |
| crates/nt-cli/src/commands/init_device.rs | wiremock-driven happy path | ❌ |
| crates/nt-cli/tests/init_device.rs | end-to-end against wiremock — exit 0, credentials file written | ❌ |
| crates/nt-cli/src/commands/init.rs | headless detection: `$DISPLAY`/`$WAYLAND_DISPLAY` absent + non-macOS → suggest device flow | ❌ |
| crates/nt-cli/src/commands/init.rs | `--web` flag forces existing callback flow even when headless detected | ❌ |
| crates/nt-cli/src/commands/init.rs | `--device` flag bypasses callback flow on any host | ❌ |

## Cross-repo dependency

Option A is blocked on server-side work in
`magic-ingredients/no-tickets-service`:

- New routes `POST /auth/device/code`, `GET /auth/device`,
  `POST /auth/device/token`
- Device-code storage (short TTL: 15 min) keyed by `device_code`
- `user_code` collision handling (regenerate on conflict; namespace
  is small)
- Token minting on successful pairing (reuses existing CLI-token
  issue path)

Open a sister fix in no-tickets-service before starting client work.

Option B's server-side surface is much smaller (one authenticated
page that mints + displays a token); could be done by anyone with
server access without a formal sister fix.

## Tasks

### 1. Open sister fix in no-tickets-service for device-code endpoints
Coordinate with no-tickets-service to land the three endpoints
described under "Cross-repo dependency". Decision gate: if the
server work won't land within the planning horizon, fall back to
Option B and skip this task — but the planning conversation has to
happen before client work starts.

**Files to modify:**
- (cross-repo) `magic-ingredients/no-tickets-service` — new fix doc
  describing the device-code endpoints

### 2. Implement `no-tickets init --device` (RFC 8628 device-code flow)
End-to-end client implementation of the device-code grant. Failing
tests against wiremock + implementation + any review-driven refactors
all land here. Uses the same credentials-file persistence as the
existing callback flow (`save_credentials` in init.rs is the shared
seam).

**Files to modify:**
- `crates/nt-cli/src/commands/init_device.rs` (new) — device-code
  request + polling loop + error mapping
- `crates/nt-cli/src/commands/init.rs` — `--device` flag, dispatch
- `crates/nt-cli/src/main.rs` — clap arg
- `crates/nt-cli/tests/init_device.rs` (new) — wiremock end-to-end
- `crates/nt-cli/Cargo.toml` — no new deps expected (reqwest already
  there)

### 3. Headless detection + UX nudges
Detect headless environments and suggest `--device` proactively
instead of trying the browser open and getting a confusing failure.
`--web` flag opts back into the existing callback flow when the
user knows tunneling is in place.

**Files to modify:**
- `crates/nt-cli/src/commands/init.rs` — detection (`$DISPLAY`,
  `$WAYLAND_DISPLAY`, `$WSL_DISTRO_NAME`, `$SSH_TTY` cases) +
  branching
- `crates/nt-cli/src/main.rs` — `--web` flag plumbing
- inline tests for the detection truth table
- `docs/install.md` — append a "Authenticating in headless
  environments" section pointing at `--device`

### 4. Update README + install.md with the new flow
Document the device-code path as the headless-default and the
loopback-callback path as the workstation default. Existing
`no-tickets init` examples in the docs continue to work — the
device path is additive.

**Files to modify:**
- `README.md` — quickstart shows `init` with a note about `--device`
  for headless
- `docs/install.md` — "Authenticating in CI" / "Authenticating in
  sandboxes" recipes
