---
id: publish-uses-push-token
title: "`no-tickets publish --project` must use the registered push token, never the session"
status: completed
severity: high
reported: 2026-05-20T00:00:00.000Z
resolved: 2026-05-20T00:00:00.000Z
resolution:
  rootCause: |
    `nt publish --project <name>` was wired against the TS-era `resolve_auth`
    fallback chain (NO_TICKETS_TOKEN env var → ~/.notickets/credentials
    session file). The `--project` flag never reached the push-token
    registry written by `token add`; it only populated source.attributes
    .project on the wire. Net effect: every publish silently sent the
    session token (a management-API identity from `init`) to /v1/events
    — privilege confusion that "worked" against a lenient server but
    401s under tightened validation.
  fix:
    - Added `auth::resolve_publish_token(env, project)` reading the
      project's push token from config.json (NO_TICKETS_TOKEN env var
      retained as CI escape hatch); session credentials never consulted
    - Rewired `commands/publish.rs` and `commands/publish_batch.rs` to
      call the new resolver instead of `resolve_auth`
    - Deleted `NotAuthenticated` variant (no production callers after
      the rewire); wire contract reservation kept in markdown
    - Deleted `NOT_AUTH_MSG`, `ResolvedAuth.token`,
      `StoredCredentials.token` would-have-been dead code (latter kept
      as #[allow(dead_code)] for serde shape validation)
    - Added new `TokenRejected` error variant (exit 8, class
      "token_rejected") for server-side 401s on requests that DID
      carry a Bearer — distinct from NotAuthenticated (reserved exit
      5 for future identity commands)
    - Hardened resolver against whitespace-only NO_TICKETS_TOKEN
      (`.trim().is_empty()`) and malformed config.json (Usage error)
    - Stripped TS-parity comments from auth.rs, publish/metadata.rs,
      publish/envelope.rs, publish_batch/jsonl.rs, publish_batch/
      source.rs
    - Updated README.md quickstart to thread `init` → `token add` →
      `publish` (the new three-step setup); docs/install.md was
      already correct
    - Updated docs/binary-error-contract.md with new token_rejected
      row, batch publish migration scope, project_not_registered vs
      token_rejected framing
    - Updated scripts/seed-product-demo.sh docstring to drop the
      legacy ~/.notickets/ path reference and clarify the token-add
      vs init distinction; default NT_BIN flipped from nt → no-tickets
  filesModified:
    - crates/nt-cli/src/auth.rs
    - crates/nt-cli/src/credentials.rs
    - crates/nt-cli/src/error.rs
    - crates/nt-cli/src/commands/publish.rs
    - crates/nt-cli/src/commands/publish/post.rs
    - crates/nt-cli/src/commands/publish/metadata.rs
    - crates/nt-cli/src/commands/publish/envelope.rs
    - crates/nt-cli/src/commands/publish_batch.rs
    - crates/nt-cli/src/commands/publish_batch/jsonl.rs
    - crates/nt-cli/src/commands/publish_batch/source.rs
    - crates/nt-cli/tests/publish.rs
    - crates/nt-cli/tests/publish/auth.rs
    - crates/nt-cli/tests/publish/error_handling.rs
    - crates/nt-cli/tests/structured_errors/publish.rs
    - docs/binary-error-contract.md
    - README.md
    - scripts/seed-product-demo.sh
---

# Fix: `publish` ignores the push-token registry; falls back to session auth

## Issue Summary

`no-tickets publish --project <name>` is supposed to bind to the push
token registered under `<name>` (via `no-tickets token add <name>`).
It doesn't. The `--project` argument is purely cosmetic for auth —
the only thing it does is populate `source.attributes.project` on the
wire (`crates/nt-cli/src/commands/publish.rs:55`).

What actually happens today (`crates/nt-cli/src/commands/publish.rs:85`):

```rust
let auth = match resolve_auth(env, &urls.api_url) {
    AuthOutcome::Resolved(a) => a,
    …
};
```

`resolve_auth` (`crates/nt-cli/src/auth.rs:59-81`) consults exactly
two sources:
1. `NO_TICKETS_TOKEN` env var
2. `~/Library/Application Support/com.magic-ingredients.no-tickets/credentials`
   — the session file written by `init`

It **never reads `config.json`** — the push-token registry that
`token add` writes and `token list` reads from.

Net effect: every `no-tickets publish` invocation sends the **session
token** to `/v1/events`, not the per-project push token. This is
privilege confusion — session credentials are a management-API
identity that carry broader authority than a per-project publish
should. A server that tightens validation will reject the session
token at the publish endpoint with a 401. Reproduced today against
staging — exit 5, `{"error":"not_authenticated","message":"server
rejected the bearer token (401)"}`.

The architectural rule (memory: `[[feedback-publish-uses-push-token-only]]`,
companion to `[[project-tokens-define-project]]`): **publish must
only ever use the push token from the registry. Session credentials
must never reach `/v1/events`.**

## Reproduction

```sh
# Mint a staging session
NO_TICKETS_ENV=staging no-tickets init    # → "Authenticated as you@example.com"

# Try to publish with --project
NO_TICKETS_ENV=staging no-tickets publish \
  --type product.epic.created.v1 \
  --data '{"epicId":"smoke","projectId":"demo","title":"hi"}' \
  --project demo
# → exit 5, {"error":"not_authenticated","message":"server rejected the bearer token (401)..."}

# Workaround: inject the push token directly
PUSH_TOKEN=$(jq -r '.projects["demo"].pushToken' \
  ~/Library/Application\ Support/com.magic-ingredients.no-tickets/config.json)
NO_TICKETS_ENV=staging NO_TICKETS_TOKEN="$PUSH_TOKEN" no-tickets publish \
  --type product.epic.created.v1 --data '…' --project demo
# → {"deduped":0,"ids":["43"],"ingested":1}  ✓
```

The fact that `NO_TICKETS_TOKEN` (escape hatch) works while `--project`
(the documented primary path) doesn't confirms the server is fine —
the gap is purely CLI-side wiring.

## Root Cause

TS-port leftover. `crates/nt-cli/src/auth.rs:2` even says so:
*"Mirrors `src/sdk/auth.ts::resolveAuth`."* The TS implementation
predates the push-token registry; when the Rust port shipped
`token add` / `token list` / `config.json` schema (`crates/nt-cli/src/config.rs`),
it didn't retrofit `publish` to consult that registry. The wiring
half-shipped. The misleading TS-parity comment in auth.rs's module
docstring probably contributed to the gap going unnoticed.

`scripts/seed-product-demo.sh:18` already assumes the correct
behavior (*"Local key from `~/.notickets/config.json` that resolves
to a push token. Passed as `nt publish --project`"*) — i.e., the
docs are right, only the code is wrong.

## Fix Approach

Replace `resolve_auth` in the publish path with a new
`resolve_publish_token(env, project)` that:

1. If `NO_TICKETS_TOKEN` env var is set + non-empty → use it.
   Escape hatch for CI; behavior unchanged.
2. Otherwise, read `config.json`, look up
   `projects[<project>].pushToken`. If found → use it.
3. Otherwise, error with a specific message:
   ```
   No push token registered for project '<name>'.
   Run: no-tickets token add <name> --token <token-from-web-ui>
   ```
   Map to a new `no_push_token` error variant + exit code, NOT
   `not_authenticated` (which today's behavior produces and is
   misleading — see the error-mapping refinement below).

Session credentials from `init` MUST NOT be consulted in this path.
The session file's purpose narrows to management-API operations
(none of which are wired today; future scope). `init` itself stays —
it's still how users prove identity to the web UI to mint push
tokens — but the credentials it writes are not a fallback for
publish.

### Error-mapping refinement (sub-issue)

When the server returns 401 on a request that DID carry a Bearer
header, mapping to `not_authenticated` (exit 5) is misleading. The
CLI knew it sent a token; the server said "no, that one's bad."
Distinct error class:

| Exit | Class | When |
|------|-------|------|
| 5 | `not_authenticated` | No token to send (no project registered, env var unset, etc.) |
| ? | `token_rejected` | Sent a token, server returned 401 |

Per Task 26's additive-only contract on `docs/binary-error-contract.md`,
add a new exit code; don't repurpose 5. Wrappers parsing exit codes
keep working.

## Test Plan

### 🔒 Regression Tests (must pass unchanged)
| File | Cases | Status |
|------|-------|--------|
| crates/nt-cli/src/auth.rs (inline) | `resolve_auth_*` for env-var / credentials / host-mismatch | ❌ |
| crates/nt-cli/tests/publish/happy_path.rs | NO_TICKETS_TOKEN env-var path continues to work | ❌ |
| crates/nt-cli/tests/publish/auth.rs (if present) | env-var beats other sources | ❌ |

### ✏️ Amended Tests
| File | Case | Change | Status |
|------|------|--------|--------|
| crates/nt-cli/tests/publish/*.rs | tests that rely on session-token-as-bearer | Update to register a push token + use --project (or set NO_TICKETS_TOKEN explicitly) | ❌ |

### 🆕 New Tests
| File | Case | Status |
|------|------|--------|
| crates/nt-cli/src/commands/publish.rs (inline) | `resolve_publish_token` reads push token from config.json by --project | ❌ |
| crates/nt-cli/src/commands/publish.rs (inline) | NO_TICKETS_TOKEN env beats config.json lookup | ❌ |
| crates/nt-cli/src/commands/publish.rs (inline) | missing project registration → typed error with `no_push_token` variant | ❌ |
| crates/nt-cli/tests/publish/no_session_fallback.rs (new) | session credentials present + push token absent for project → error, NOT fallback to session | ❌ |
| crates/nt-cli/tests/publish/no_session_fallback.rs | session credentials present + push token also present → push token wins (no leak of session to `/v1/events`) | ❌ |
| crates/nt-cli/tests/structured-errors.rs | new `token_rejected` exit code + stderr shape for 401 with token | ❌ |

## Tasks

### 1. Replace `resolve_auth` in publish path with push-token-only resolver
status: completed
commitSha: 212f66c

End-to-end task: failing tests + implementation + review-driven refactors
land here. Introduce `resolve_publish_token(env, project)` in either
`auth.rs` or a new `publish/token.rs`; rewire `publish.rs:85` to call
it instead of `resolve_auth`. Session credentials are no longer
consulted by the publish path. `NO_TICKETS_TOKEN` env-var escape hatch
stays.

**Files to modify:**
- `crates/nt-cli/src/auth.rs` (or new `publish/token.rs`) — new
  resolver function
- `crates/nt-cli/src/commands/publish.rs:85` — swap call site
- `crates/nt-cli/src/error.rs` — new `NoPushToken { project }` variant
- `crates/nt-cli/src/config.rs` — possibly expose a
  `find_project(name) -> Option<ProjectEntry>` helper if not already
- `crates/nt-cli/tests/publish/no_session_fallback.rs` (new)
- relevant existing tests under `crates/nt-cli/tests/publish/`
  that need amending

### 2. Distinguish `token_rejected` from `not_authenticated` in error contract
status: completed
commitSha: c565e67

End-to-end task. New exit code + stderr JSON shape for the
"server rejected our token" case; keep exit 5 for
"no token was sent". Additive per `docs/binary-error-contract.md`'s
contract guarantee.

**Files to modify:**
- `crates/nt-cli/src/error.rs` — new variant, exit-code mapping
- `crates/nt-cli/src/commands/publish.rs` — 401-with-token branch
  emits the new variant instead of `not_authenticated`
- `crates/nt-cli/tests/structured-errors.rs` — assertions per
  Task 26's table
- `docs/binary-error-contract.md` — append the new exit code row

### 3. Strip TS-parity comments from auth.rs while we're touching it
status: completed
commitSha: 212f66c

Memory `[[feedback-no-ts-references-in-rust]]` applies: the
`Mirrors src/sdk/auth.ts::resolveAuth` line in `auth.rs:2` (and any
others in the publish auth surface) should go when this work lands.
Folded into the refactor commit on Task 1 rather than a separate
cycle.

**Files to modify:**
- `crates/nt-cli/src/auth.rs` — strip TS-parity docstrings
- any other auth-surface files touched during Task 1 that still
  carry TS_PARITY identifiers / comments

### 4. Update docs to reflect the corrected publish auth model
status: completed
commitSha: pending

After Task 1 + 2 land, document the rule clearly: publish uses
the push token registered for `--project`, full stop. Mention the
new `token_rejected` exit code so wrappers know to handle it.

**Files to modify:**
- `README.md` — clarify in quickstart that publish needs a push
  token via `token add` first
- `docs/install.md` — same in any publish recipes
- `docs/binary-error-contract.md` — already covered in Task 2,
  but cross-link from here

### 5. Strip dead `~/.notickets/` path references from docs + scripts
status: completed
commitSha: pending

Investigation side-discovery: the Rust binary on macOS uses
`~/Library/Application Support/com.magic-ingredients.no-tickets/`
(via `directories::ProjectDirs` in `crates/nt-cli/src/paths.rs:31-36`)
and the equivalent XDG / `%APPDATA%` paths on Linux / Windows. The
legacy `~/.notickets/` location is **dead TS state** — the Rust
binary never reads or writes it. But documentation + helper scripts
still reference it, which sends users (and AI agents) hunting in the
wrong place when debugging auth issues.

Known references to sweep:
- `scripts/seed-product-demo.sh:18` — docstring says *"Local key from
  `~/.notickets/config.json` that resolves to a push token"*
- `crates/nt-cli/src/credentials.rs` — module docstring mentions the
  credentials file but doesn't pin the path; check for any literal
  `~/.notickets/` strings
- `README.md` / `docs/install.md` — grep for `~/.notickets` and
  rewrite as either platform-native examples or `no-tickets token
  list` / `status` invocations that don't expose the path

Replacement strategy: where docs need to reference the config or
credentials location, prefer pointing the reader at the CLI command
that touches it (`no-tickets token list`, `status`, etc.) rather than
the file path. The platform-native path is a `directories`-crate
implementation detail; cementing it in docs makes future changes
(e.g., XDG opt-in on macOS) harder.

Where path examples are genuinely useful (e.g., debugging recipes),
use `no-tickets status --paths` if it exists, or document the
per-platform paths together rather than only `~/.notickets/`.

**Files to modify:**
- `scripts/seed-product-demo.sh` — docstring at line 18
- `README.md` — grep + rewrite
- `docs/install.md` — grep + rewrite
- `crates/nt-cli/src/credentials.rs` — if any literal path comments
- any other `~/.notickets/` references the sweep surfaces
