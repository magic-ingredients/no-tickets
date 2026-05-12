---
id: implement-adr-0002-cli-surface
title: "Implement ADR-0002 — reshape CLI surface, two-tier auth, platform-native paths"
status: completed
severity: medium
reported: 2026-05-11T00:00:00.000Z
resolved: 2026-05-12T00:00:00.000Z
resolution:
  rootCause: "TS CLI surface accumulated four ADR-0002 frictions: misnamed nt-project-link, 401-on-stale-session nt-token-list, leaky --profile flag, and POST /v1/tokens callable by any bearer session credential. The Rust port was the reshape point — backcompat already dropped per the rewrite memo."
  fix:
    - "Platform-native config dir via directories crate (was ~/.notickets/)"
    - "Three-layer URL resolution (defaults / NO_TICKETS_ENV preset / explicit pair); --profile plumbing deleted"
    - "Session credentials gain host field; mismatch surfaces stderr warning and declines session"
    - "config.json flattened to projects/<name> with #[serde(flatten)] extras preserving unknown keys; atomic 0600 writes"
    - "nt token add/list/remove commands replace nt project link/list/unlink and nt token create/revoke"
    - "nt status reshaped to {authenticated, email?, tokens} per the four ADR scenarios"
    - "nt init ported (browser + local HTTP callback server, CSRF nonce); nt logout added (delete credentials file)"
  filesModified:
    - crates/nt-cli/src/paths.rs
    - crates/nt-cli/src/urls.rs
    - crates/nt-cli/src/credentials.rs
    - crates/nt-cli/src/auth.rs
    - crates/nt-cli/src/config.rs
    - crates/nt-cli/src/auth_server.rs
    - crates/nt-cli/src/commands/init.rs
    - crates/nt-cli/src/commands/logout.rs
    - crates/nt-cli/src/commands/status.rs
    - crates/nt-cli/src/commands/token_add.rs
    - crates/nt-cli/src/commands/token_list.rs
    - crates/nt-cli/src/commands/token_remove.rs
    - crates/nt-cli/src/main.rs
---

# Fix: Implement ADR-0002 — CLI surface and token lifecycle

## Issue Summary

**Reported:** 2026-05-11
**Severity:** medium

[ADR-0002](../../docs/adr/0002-cli-surface-and-token-lifecycle.md) defines the new CLI surface (two-tier auth, paste-only push tokens, no profile concept, platform-native storage). This fix drives the implementation across the Rust port (`nt-cli`), the existing TS CLI (deletions), and identifies the cross-repo work in `no-tickets-service` and the web UI that this fix depends on for end-to-end correctness.

This fix touches **client only**. The server endpoint moves (`POST /v1/tokens` and `DELETE /v1/tokens/{id}` out of bearer-token auth class) and the web UI snippet emission are filed as separate fixes against `no-tickets-service` (see "Cross-repo dependencies" below) — those are *blocking* for the security posture to actually be achieved end-to-end, but the client-side work can proceed in parallel since the new client doesn't call those endpoints regardless.

## Root Cause Analysis

The current TS CLI surface accumulated incrementally and carries four real frictions (documented in detail in ADR-0002):

1. `nt project link` is a wrongly-named verb that exposes implementation rather than user intent.
2. `nt token list` 401s on stale session tokens (it queries `/v1/tokens`).
3. `--profile` leaks an internal-team concept onto every user-facing help text and doc.
4. `POST /v1/tokens` is a privilege-escalation primitive callable by any compromised session credential.

The Rust port (in progress under `cross-platform-cli-binary`) is the right moment to reshape — backcompat is already being dropped per the rewrite memo, no migration debt is incurred.

## Fix Approach

Six tasks, ordered so each one can land independently without breaking the prior:

1. **Storage paths** — move from `~/.notickets/` to platform-native via `directories` crate. Smallest blast radius; everything else depends on the new path resolution.
2. **URL resolution** — replace today's `--profile` plumbing with the three-layer model (defaults → `NO_TICKETS_ENV` → explicit pair). Drops ~120 LOC + 6 error variants from `urls.rs`.
3. **Credentials file shape** — add `host` tag to detect env-mismatch. Touch the session-loading path only; pure additive.
4. **Config registry** — flatten `~/.notickets/config.json` to `projects.<name> = { pushToken, addedAt, label }`. Drop the `profiles` top-level section.
5. **CLI verbs** — implement `nt token add`, `nt token list`, `nt token remove`. Wire `nt status` to the new combined view. Delete `nt project *` plumbing and the `--profile` flag from clap.
6. **`nt logout`** — new verb that deletes credentials. Symmetric with `nt init`.

The currently-superseded TS CLI source (`src/cli.ts`, `src/commands/*.ts`, etc) stays untouched until `cross-platform-cli-binary` Task 12 retires it wholesale — no half-deletes.

## Test Plan

### 🔒 Regression Tests (must pass unchanged)

| File | Cases | Status |
|------|-------|--------|
| `crates/nt-cli/src/**/*` inline `#[cfg(test)]` | the 43 unit tests from `nt-cli-thin-edge-refactor` | ❌ |
| `crates/nt-cli/tests/publish.rs` | all 11 wiremock cases (push-token publish unchanged) | ❌ |
| `crates/nt-mcp/tests/mcp.rs` | unchanged | ❌ |
| `crates/nt-schemas/tests/validate.rs` | unchanged | ❌ |

The 31 cases in `crates/nt-cli/tests/status.rs` mostly delete or change shape (~15 `status_profile_*` deletions per ADR; the rest reshape per the new `nt status` output). These are not regression tests post-this-fix — they're being replaced.

### ✏️ Amended Tests

| File | Case | Change | Status |
|------|------|--------|--------|
| `crates/nt-cli/tests/status.rs` | (multiple) | Replace `status_profile_*` set with new `status_with_session_*` / `status_without_session_*` tests covering the four output scenarios from the ADR | ❌ |
| `crates/nt-cli/tests/publish.rs` | All | Update test fixtures to use platform-native storage paths via `NO_TICKETS_HOME` (already used today; semantics unchanged) | ❌ |

### 🆕 New Tests

| File | Case | Status |
|------|------|--------|
| `crates/nt-cli/src/paths.rs` (new, inline) | platform-native resolution returns the directories-crate path | ❌ |
| `crates/nt-cli/src/paths.rs` (new, inline) | `NO_TICKETS_HOME` override wins over platform-native | ❌ |
| `crates/nt-cli/src/urls.rs` (inline) | three-layer resolution: defaults / `NO_TICKETS_ENV=staging` / explicit pair | ❌ |
| `crates/nt-cli/src/urls.rs` (inline) | `NO_TICKETS_ENV=unknown` returns `UnknownEnv` error | ❌ |
| `crates/nt-cli/src/urls.rs` (inline) | `NO_TICKETS_ENV` AND explicit pair both set → mutual-exclusion error | ❌ |
| `crates/nt-cli/src/credentials.rs` (inline) | session-host field round-trips through save → load | ❌ |
| `crates/nt-cli/src/credentials.rs` (inline) | session loaded with mismatched `host` vs current env → treated as None (mismatch warning at caller) | ❌ |
| `crates/nt-cli/src/config.rs` (new, inline) | flat `projects.<name> = { pushToken, addedAt, label }` round-trips | ❌ |
| `crates/nt-cli/src/config.rs` (inline) | masking helper: `nt_push_a0e7...` → `nt_push_…<last4>` | ❌ |
| `crates/nt-cli/src/commands/token_add.rs` (new, inline + integration) | adds token to config; refuses overwrite without `--force` | ❌ |
| `crates/nt-cli/src/commands/token_add.rs` | rejects non-`nt_push_*` token prefix | ❌ |
| `crates/nt-cli/src/commands/token_add.rs` | `--label "free text"` flag stored verbatim | ❌ |
| `crates/nt-cli/src/commands/token_list.rs` (new, inline + integration) | empty config → `{ "tokens": [] }` | ❌ |
| `crates/nt-cli/src/commands/token_list.rs` | populated config → masked tokens + addedAt + label | ❌ |
| `crates/nt-cli/src/commands/token_remove.rs` (new, inline + integration) | removes entry; missing project errors out cleanly | ❌ |
| `crates/nt-cli/src/commands/status.rs` (inline + integration) | the four scenarios from the ADR (`no session / no tokens`, `no session / tokens`, `session / tokens`, `session-host mismatch`) | ❌ |
| `crates/nt-cli/src/commands/logout.rs` (new, inline + integration) | removes credentials file; no-op when absent | ❌ |
| `crates/nt-cli/tests/cli_surface.rs` (new) | `nt --help` does NOT mention `--profile`, `project`, `token create`, `token revoke` | ❌ |

## Tasks

### 1. Switch on-disk paths to platform-native via `directories` crate
status: completed
commitSha: faa7ebf

End-to-end task: introduce `crates/nt-cli/src/paths.rs` (or fold into `home.rs` and rename) wrapping `ProjectDirs::from("com", "magic-ingredients", "no-tickets")`. Replace `home::credentials_path` / `home::config_path` call sites. Preserve `NO_TICKETS_HOME` override semantics (env var subsumes platform-native). Add inline unit tests for both branches via the existing `HashMapEnv`. Existing `tests/publish.rs` and `tests/status.rs` continue to pass — they already use `NO_TICKETS_HOME` for isolation, so they're unaffected by the platform-native default.

**Files to modify:**
- `crates/nt-cli/Cargo.toml` — add `directories = "5"` dep
- `crates/nt-cli/src/paths.rs` (new) — `config_dir(env) -> Option<PathBuf>` + `credentials_path(env)` + `config_path(env)` helpers
- `crates/nt-cli/src/home.rs` — delete or shrink to a re-export shim
- `crates/nt-cli/src/credentials.rs` — call new `paths::credentials_path`
- `crates/nt-cli/src/urls.rs` — call new `paths::config_path` (for now; deleted in Task 2)

### 2. Replace `--profile` plumbing with three-layer URL resolution
status: completed
commitSha: 3892e86

End-to-end task: rewrite `crates/nt-cli/src/urls.rs` per the ADR. Three layers: default URLs, `NO_TICKETS_ENV` preset (closed set: `staging` / `local` / `prod`), explicit `NO_TICKETS_API_URL` + `NO_TICKETS_AUTH_URL` pair (both required if either set). Delete `load_profile`, `ConfigFile`, `ProfileConfig`, `IndexMap` import, and the 4 profile-related error variants (`ProfileFileMissing`, `ProfileFileUnreadable`, `ProfileFileInvalidJson`, `ProfileNotFound`, `ProfileInvalidUrls`). Keep `PartialPair` and `HomeUnresolvable` (rename latter if no longer applicable). Add `UnknownEnv { value }`. Delete `--profile` flag from clap in `main.rs`. Delete the ~15 `status_profile_*` integration tests; replace with three `status_env_*` unit tests inline.

**Files to modify:**
- `crates/nt-cli/src/urls.rs` — rewrite per ADR
- `crates/nt-cli/src/main.rs` — drop `#[arg(long, global = true)] profile` from clap struct
- `crates/nt-cli/src/commands/status.rs` — drop `profile` arg from `run` signature
- `crates/nt-cli/src/commands/publish.rs` — drop `profile` from `PublishArgs`
- `crates/nt-cli/tests/status.rs` — delete `status_profile_*`, `status_emits_default_urls_when_no_env_no_profile` (rename), `status_uses_env_urls_when_both_set`, etc.

### 3. Add `host` tag to session credentials with mismatch detection
status: completed
commitSha: 39ff5f4

End-to-end task: extend `StoredCredentials` (in `crates/nt-cli/src/credentials.rs`) with a `host` field. Set it at `nt init` save-time to the api_url that was used for the auth flow. On `load(env)`, compare against current env's resolved api_url; if mismatched, return None and surface a stderr warning at the caller (`nt status` and any future identity-aware command). Existing credential files without the `host` field load as None (forces re-init) — clean degradation, no schema-migration code.

**Files to modify:**
- `crates/nt-cli/src/credentials.rs` — add `host: String` field; load-side mismatch detection
- `crates/nt-cli/src/commands/init.rs` (new — port from TS `src/commands/init-auth.ts`) — save `host` from the resolved env at save-time
- `crates/nt-cli/src/commands/status.rs` — surface the mismatch warning to stderr

### 4. Flatten config.json to flat `projects` registry
status: completed
commitSha: ccd6039

End-to-end task: introduce `crates/nt-cli/src/config.rs` owning the new config shape. Per-project entry: `{ pushToken, addedAt: ISO-string, label?: string }`. Delete the `profiles` top-level concept entirely. Add a `mask_token` helper that returns `nt_push_…<last4>`. Atomic-write semantics (sibling tmp + rename, mode 0600) per the existing TS `writeConfigSync`. Unknown top-level keys preserved unchanged on rewrite — so any user with old `profiles` data isn't silently corrupted.

**Files to modify:**
- `crates/nt-cli/src/config.rs` (new) — `read(env) -> Result<Config>`, `write(env, &Config) -> Result<()>`, `mask_token`
- `crates/nt-cli/Cargo.toml` — add `time` features for ISO parsing if not already present

### 5. Implement `nt token add / list / remove` and reshape `nt status`
status: completed
commitSha: 77cef37

End-to-end task: three new command modules under `crates/nt-cli/src/commands/`. `token_add` validates the `nt_push_*` prefix, refuses overwrite without `--force`, accepts optional `--label`. `token_list` prints the flat JSON shape (`{ tokens: [{ project, masked, addedAt, label? }, ...] }`). `token_remove` errors cleanly on missing project. `status` combines session + tokens per the four ADR scenarios.

**Files to modify:**
- `crates/nt-cli/src/commands/token_add.rs` (new)
- `crates/nt-cli/src/commands/token_list.rs` (new)
- `crates/nt-cli/src/commands/token_remove.rs` (new)
- `crates/nt-cli/src/commands/mod.rs` — register new modules
- `crates/nt-cli/src/commands/status.rs` — reshape per ADR
- `crates/nt-cli/src/main.rs` — clap definitions for `token add/list/remove`, drop `project` subcommand surface

### 6. Implement `nt init` (port) + `nt logout`
status: completed
commitSha: bf41b3d

End-to-end task: port `src/sdk/auth-server.ts` to Rust (local HTTP callback server with CSRF state, timeout, signal-handler dance). New `commands/init.rs` orchestrates the flow, saves credentials with `host` tag. New `commands/logout.rs` deletes the credentials file.

This is the largest sub-task (~250 LOC + tests for the auth server alone). Reasonable to land last since the publish path doesn't depend on it.

**Files to modify:**
- `crates/nt-cli/src/auth_server.rs` (new) — local HTTP callback server
- `crates/nt-cli/src/commands/init.rs` (new)
- `crates/nt-cli/src/commands/logout.rs` (new)
- `crates/nt-cli/Cargo.toml` — add `hyper` or `tiny_http` for the local server
- `crates/nt-cli/src/main.rs` — clap definitions for `init` + `logout`

## Cross-repo dependencies (blocking for end-to-end correctness)

These are NOT in scope for this fix but are required for the ADR's security posture to take effect server-side:

1. **`no-tickets-service` — move `POST /v1/tokens` out of bearer auth class.** UI-only via cookie session. Without this, a leaked client session token can still mint push tokens, defeating ADR-0002's Friction 4 mitigation.
2. **`no-tickets-service` — move `DELETE /v1/tokens/{id}` out of bearer auth class.** Same reasoning, destructive endpoint.
3. **`no-tickets-service` — add `GET /v1/me` and `GET /v1/projects` to bearer auth class** (if not already there). Required for future `nt me` / `nt project list` commands.
4. **Web UI — emit `nt token add ...` snippet on token creation.** Replaces the today's two-step setup flow with one paste.

Each of those is a separate fix in the relevant repo. Coordinate timing so the client surface change and server endpoint moves ship in the same release window — otherwise either:
- Server moves first: today's CLI breaks (`POST /v1/tokens` 404 on `nt token create`). Acceptable per no-backcompat.
- Client moves first: new CLI works, but server still permits the privilege-escalation primitive — security posture not yet achieved.

**Recommendation:** server endpoint moves ship before this fix lands its release tag, so the security posture is in effect from day one of the new CLI surface being available.
