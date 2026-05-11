---
adr_number: 2
title: "CLI surface and token lifecycle: two-tier auth, paste-only push tokens, no profile concept"
date: 2026-05-11
status: proposed
supersedes: null
superseded_by: null
tags: [architecture, cli, security, tokens, surface]
decision_makers: [Andy Richardson]
---

# ADR-0002: CLI surface and token lifecycle

## Status

Proposed

## Context

The TS CLI's command surface accumulated incrementally and has grown four frictions worth solving before Task 5 of `cross-platform-cli-binary` (full Rust port) builds more code on the existing foundation. The Rust port is the right moment to reshape — backcompat with `@magic-ingredients/no-tickets` consumers is already being dropped (per the `event-repository-foundation` rewrite memo), so no migration debt is incurred.

### Friction 1 — `nt project link` is wrongly-named

Today's flow to register a token for use:

```bash
nt project link mystaging --profile staging --token nt_push_a0e79856...
```

The verb "link" describes the implementation (linking a project name to a profile + token) rather than the user intent (adding a token). Users have to learn three nouns — *project*, *profile*, *token* — to perform what is conceptually one action: "I have a token; let my CLI use it for project X."

### Friction 2 — `nt token list` 401s on stale session tokens

`nt token list` calls `GET /v1/tokens` authenticated with the session token stored in `~/.notickets/credentials` (lifetime: 7 days from `nt init`). When the session expires, every `token list` invocation returns 401. The user has to re-run `nt init` to do a *local* operation — listing what's in their own config file — because the command is implemented as a remote query.

### Friction 3 — `--profile` leaks an internal concept

The `--profile` flag exists so the no-tickets team can test publishes against `app-staging.no-tickets.com` from their own machines. **For every external consumer, the destination is always `api.no-tickets.com`.** The flag is in every user-facing help text, every command, every documentation example — leaking a concept users never benefit from.

### Friction 4 — `POST /v1/tokens` is a privilege-escalation primitive

The session token sitting in `~/.notickets/credentials` (file mode 0600, ~7 day lifetime) can call `POST /v1/tokens` to mint new push tokens silently. Anyone with read access to that file (compromised laptop, sloppy backup, leaky CI cache, malicious npm dep in the node_modules of any tool the user runs) can mint push tokens that never appear in the web UI's "tokens I created" list — because the UI shows tokens you created via the UI, not via the API. The audit story is broken by design.

### Industry precedent — provisioning

| Service | API endpoint to mint user tokens? | Notes |
|---|---|---|
| **GitHub** | ❌ No | PATs created exclusively in web UI. `POST /authorizations` deprecated 2020, removed shortly after. Org-side `personal-access-token-requests` endpoints are for *approval* workflows, not creation. |
| **Railway** | ❌ No | Account / workspace / project tokens dashboard-only. GraphQL API uses tokens but exposes no `createToken` mutation. |
| **Stripe** | ❌ No | API keys created in the dashboard only. |
| **Anthropic** | ❌ No | API keys created in console only. |
| **Vercel** | ✅ Yes | `POST /v3/user/tokens` mints tokens via API. The outlier. |
| **AWS IAM** | ✅ Yes | `iam:CreateAccessKey` — existing key mints more. |

The "API can mint tokens" services have programmatic provisioning use cases that justify the security trade-off (preview deployments, sub-system credentials). The "UI only" services chose the security posture because token-leak blast radius matters more than convenience for one-time setup. `no-tickets`' use case — humans + AI agents publishing events — has no programmatic-provisioning need. UI-only camp.

### Industry precedent — ingest auth

Telemetry products universally use single-key auth per request; "session bearer + ingest token" layering does not appear in this category:

| Service | Ingest auth | Notes |
|---|---|---|
| **Datadog** | `DD-API-KEY` header (write/ingest) + `DD-APPLICATION-KEY` (read/queries) | Split-permission keys, not layered. API key alone suffices for ingest. |
| **Sentry** | DSN — single project-scoped key embedded in client | Optional secret half deprecated. |
| **Honeycomb** | Ingest-Only API Keys — write-only, environment-scoped, immutable permissions | Marketed as "the safest option for client-side instrumentation." |
| **PostHog** | Project API keys — single key, write-scoped | |
| **Segment** | Write keys per source — single key | |
| **New Relic** | License keys (ingest) + User API keys (queries) | Split-permission, single key per request. |
| **Mixpanel** | Project tokens — single key for client-side ingest | |

The recurring pattern: single long-lived key per request, with security derived from scope minimisation (write-only ingest keys), project/environment scoping (one key per project), and easy rotation (mint new → revoke old, no session-refresh dance). Adding a session bearer in front of an ingest key doesn't raise security — an attacker who reads the credentials file gets both. Where session + bearer *does* buy security (financial APIs, AWS STS, OAuth refresh flows) the data is sensitive, mutable, or cross-account reachable. Telemetry data is the opposite — append-only, project-scoped, recovery is "revoke key, ignore garbage window."

### Industry precedent — CLI identity

CLIs that need user identity for richer commands (not just ingest) keep a session-style credential separate from per-resource tokens:

- **`gh` (GitHub CLI)**: `gh auth login` establishes a session used by `gh pr`, `gh issue`, `gh repo`, etc. `git push` over HTTPS uses a PAT (paste) or a token derived from `gh`'s session. Two auth concerns, separated, both legitimate.
- **`vercel`**: `vercel login` for interactive operations; `VERCEL_TOKEN` for CI/scripted ingest. Same shape.
- **`gcloud`**: `gcloud auth login` for user, service-account keys for ingest/automation.

The split is consistent: session for identity-aware *reads* and *user-bound mutations*; long-lived per-resource keys for high-volume *ingest*.

## Decision

Two-tier auth, with strict server-side separation between which endpoints accept which credential type.

### Tier 1 — Session bearer (`nt_session_*`)

- **Acquired via** `nt init` (browser callback flow — unchanged).
- **Stored in** `~/.notickets/credentials`, mode 0600.
- **Lifetime** 7 days (server-issued expiry, tracked locally).
- **Capability** read-only / non-destructive identity-aware operations against the user's account.

### Tier 2 — Push token (`nt_push_*`, project-scoped)

- **Acquired via** web UI mint → `nt token add` paste. No CLI-side mint flow.
- **Stored in** `~/.notickets/config.json` under `projects.<name>`.
- **Lifetime** server-enforced expiry (e.g. 90 days, surfaced via 401 at use-time).
- **Capability** publish to one project's event stream. Write-only, project-scoped, append-only target.

### Server endpoint authentication classes

The security boundary moves from "session token exists" to "which endpoints accept the bearer session credential":

| Endpoint | Auth class | Rationale |
|---|---|---|
| `POST /v1/events` | Bearer **push token** | High-volume ingest; project-scoped writes. |
| `GET /v1/me` | Bearer **session** | Identity read. |
| `GET /v1/projects` | Bearer **session** | List projects user has access to. |
| `GET /v1/event-types` | Bearer **session** OR push token | Registry browse; push token works for publish-time schema discovery. |
| `GET /v1/tokens` | Bearer **session** | List server-side tokens (consumed by web UI; CLI doesn't call this). |
| `POST /v1/tokens` | **UI-only via cookie session** — removed from bearer auth class | Privilege-escalation primitive; UI-only restricts to browser sessions which are not stealable via file read. |
| `DELETE /v1/tokens/{id}` | **UI-only via cookie session** — removed from bearer auth class | Destructive; same reasoning. |

Critical property: the session token's blast radius if leaked = **read access to your account metadata**. It cannot mint new credentials, cannot delete data, cannot publish events. That's a meaningful step down from today, where a leaked session can mint push tokens that never appear in the UI's audit log.

### CLI surface

```
nt init                                     — browser-auth flow, fetches session for identity-aware operations
nt logout                                   — delete ~/.notickets/credentials (clean parallel to init)

nt publish <type> <data> --project <name>   — push-token auth (zero session involvement)
nt validate <type> <data>                   — local schema validation, no auth

nt status                                   — combines local token registry view + session identity if init'd

nt token add <project> <pushToken> [--label <text>]
                                            — paste from UI; pure local config write
nt token list                               — list locally registered tokens (project + masked token + addedAt + label)
nt token remove <project>                   — remove from local config (does NOT revoke server-side)

— future commands (require session, all read-only):
nt project list                             — projects you have access to
nt event-types list                         — registry browse
nt me                                       — identity check
```

### Deleted from today's surface

- ❌ `nt token create` — server endpoint moves out of bearer auth class.
- ❌ `nt token revoke` (server-side semantics) — destructive endpoint moves out of bearer auth class. Renamed to `nt token remove` with strictly local semantics.
- ❌ `nt project link / list / unlink` — folded into the `nt token` verbs.
- ❌ `--profile` flag everywhere.
- ❌ Top-level `profiles` section from `~/.notickets/config.json`.

### `nt status` behaviour

| Scenario | Output |
|---|---|
| No session, no tokens | `{ "authenticated": false, "tokens": [] }` |
| No session, tokens registered | `{ "authenticated": false, "tokens": [{project, masked, addedAt, label}, ...] }` |
| Session + tokens | `{ "authenticated": true, "email": "x@y.com", "tokens": [...] }` |
| Session-host / current-env mismatch | `authenticated: false`, plus warning to stderr: "Credentials are for X; current env points to Y. Run `nt init` to re-authenticate." |

The "publish only" user (CI runner, bot) skips `nt init` entirely and uses `nt token add`. The "interactive developer" runs `nt init` once for richer commands. Both work; neither is mandatory for the other.

### On-disk storage paths

Platform-native via the `directories` crate (`ProjectDirs::from("com", "magic-ingredients", "no-tickets")`). Modern CLIs (`gh`, `vercel`, `gcloud` post-2020) follow this convention; legacy Unix-style (`~/.aws/`, `~/.docker/`) is retained only for backcompat in older tools.

| Platform | Config + credentials directory |
|---|---|
| **Linux** | `$XDG_CONFIG_HOME/no-tickets/` (falls back to `~/.config/no-tickets/`) |
| **macOS** | `~/Library/Application Support/com.magic-ingredients.no-tickets/` |
| **Windows** | `%APPDATA%\magic-ingredients\no-tickets\config\` |

Both `credentials` and `config.json` live in the platform-native config directory. (No separate "data" / "cache" dirs needed for this scope — the auth-server callback HTML is rendered in-memory; nothing else persists.)

**Override:** `NO_TICKETS_HOME=<dir>` env var. When set, the binary reads / writes inside `<dir>/.notickets/` instead of the platform-native location. Used by:
- The test suite, for isolation (per `crates/nt-cli/tests/status.rs`, `tests/publish.rs`).
- Rare power-users who want explicit control over credential location (e.g. encrypted volumes).

**Migration from today's `~/.notickets/`:** none. Per the existing no-backcompat memo for the rewrite, users re-`nt init` and re-`nt token add` after upgrading. No migration code; ~30 LOC of save-future-cleanup is not justified for a one-time transition.

### Token storage — encryption posture

Push tokens are stored **plaintext** in `config.json`, file mode 0600 (owner read/write only). Same posture as `gh`, AWS CLI, `gcloud`, `kubectl`, `git` HTTPS credentials.

Rationale for plaintext-default:
- Push tokens are **write-only**, **project-scoped**, target an **append-only event stream**. Worst-case leak = "attacker writes garbage to one project's event log" — fully recoverable via token revoke + new token, no data exfiltrated, no destructive operations possible.
- Filesystem ACLs (`0600` + user-owned home dir) are the protection layer every comparable dev-tool CLI relies on.
- OS keychain integration (macOS Keychain / Linux Secret Service / Windows Credential Manager) is the next tier up, but adds ~200 LOC of per-platform code, CI/headless fallback logic, and a "keychain daemon not running" failure mode. Not justified for tokens with this blast radius.

**Future opt-in** (out of scope for this ADR): a `--secure` flag at add-time that stores the secret in the OS keychain, with `config.json` holding metadata only (`{project, addedAt, label, keychainRef}`). Plaintext stays the default.

### On-disk data model

**`credentials`** (in the platform config dir) — session-only:

```json
{
  "token": "nt_session_...",
  "email": "user@example.com",
  "expiresAt": "2026-05-18T20:09:00.000Z",
  "host": "https://api.no-tickets.com"
}
```

The `host` field tags which environment the session was issued against. If the current env (per URL resolution below) doesn't match, the binary treats the session as invalid and prompts re-init. **Option A** (single-session, current-env-wins) — no per-env credential slots. Internal team eats the one-extra-`nt init` per env-switch (small cost, simple model).

**`config.json`** (in the platform config dir) — token registry only, no profiles:

```json
{
  "projects": {
    "mystaging": {
      "pushToken": "nt_push_a0e79856...",
      "addedAt": "2026-05-11T20:09:00.000Z",
      "label": "personal staging"
    }
  }
}
```

Per-project fields:
- `pushToken` (required) — secret; masked in all display output to `nt_push_…<last4>`.
- `addedAt` (required) — ISO timestamp set at `nt token add` time.
- `label` (optional) — free-text descriptor supplied via `--label`.

**Notably absent from the token entry:**
- No `profile` field — every token publishes to the env-resolved URL.
- No `apiUrl` field per token — same.
- No `expiresAt` per token — server enforces, surfaces via 401 at use-time. Asking the user to type `--expires` at paste time is unjustified friction.
- No `lastUsed` per token — would add write traffic on every publish for no actionable benefit.
- No server-side `id` — only useful for server-revoke, which the CLI no longer does.

### URL resolution (three layers)

```rust
pub fn resolve_urls(env: &dyn Env) -> Result<ResolvedUrls, UrlError> {
    let api = env.var("NO_TICKETS_API_URL").filter(|s| !s.trim().is_empty());
    let auth = env.var("NO_TICKETS_AUTH_URL").filter(|s| !s.trim().is_empty());

    // Layer 3 — explicit URL pair (escape hatch). Both required if either is set.
    match (api, auth) {
        (Some(api), Some(auth)) => return Ok(ResolvedUrls { api_url: api, auth_url: auth }),
        (Some(_), None) | (None, Some(_)) => return Err(UrlError::PartialPair { ... }),
        (None, None) => {}
    }

    // Layer 2 — single-knob env preset.
    match env.var("NO_TICKETS_ENV").as_deref() {
        Some("staging") => Ok(STAGING_URLS),
        Some("local")   => Ok(LOCAL_URLS),
        Some("prod") | None => Ok(PROD_URLS),  // Layer 1 — default
        Some(unknown) => Err(UrlError::UnknownEnv { value: unknown.to_string() }),
    }
}
```

| Layer | Mechanism | Audience | Use case |
|---|---|---|---|
| 1 | Default `api.no-tickets.com` + `app.no-tickets.com/api/auth/cli` | 99.99% of consumers | Production publish |
| 2 | `NO_TICKETS_ENV=staging\|local\|prod` (single knob, closed preset table) | no-tickets internal team | Daily staging testing |
| 3 | `NO_TICKETS_API_URL` + `NO_TICKETS_AUTH_URL` (both required) | no-tickets internal team | Branch-deploy / ad-hoc / PR preview testing |

Layers 2 and 3 are mutually exclusive: setting both is an error ("set either NO_TICKETS_ENV or both URL vars, not both"). Layer 3 retains the partial-pair error from today.

For the no-tickets team's staging workflow, a one-line shell helper:

```bash
ntstg() { NO_TICKETS_ENV=staging nt "$@"; }
```

### Web UI add-flow

The "create token" page must emit a copy-pasteable one-liner after token creation:

```
To use this token from the CLI, paste:

  nt token add myproject nt_push_a0e79856da36a60367c38def8ccac62e85b79d81a46863338b21fe86f29ae0c9
```

Replaces the today's two-step `nt init` + `nt project link` for the publish-only setup case.

## Consequences

### Positive

- **Privilege-escalation primitive removed.** Compromised session credentials cannot mint push tokens or destroy server-side tokens. Cookie-session auth for the UI-only endpoints is not file-readable.
- **Surface area reduces ~40%.** From `init / status / publish / validate / logout / token (add/create/list/revoke) / project (link/list/unlink)` (≈11 commands) to `init / logout / status / publish / validate / token (add/list/remove)` (≈7 commands), with room for future identity-aware reads (`project list`, `event-types list`, `me`).
- **`nt token list` becomes infallible** — local config printer, no network, no auth, no stale-session bug.
- **One-paste setup** for publish-only users — UI emits the `nt token add ...` snippet directly.
- **Rust port simplified significantly:**
  - `urls.rs` collapses from ~150 LOC + 6 error variants to ~30 LOC + 2 error variants.
  - The whole `~/.notickets/config.json` `profiles` loader (IndexMap, JSON shape validation, `ProfileFileMissing` / `ProfileFileUnreadable` / `ProfileFileInvalidJson` / `ProfileNotFound` / `ProfileInvalidUrls`) deletes.
  - ~15 `status_profile_*` integration tests delete.
  - `--profile` flag deletes from clap.
- **No internal concept leaks into user-facing surface.** Consumers never see `profile` or `NO_TICKETS_ENV`. Internal-team overrides live in undocumented (or lightly documented) env vars.
- **`nt status` becomes more useful** — combines local registry view with session identity if available; clearly degrades when session is absent or env-mismatched.

### Negative / trade-offs

- **No CLI-side programmatic token provisioning.** Teams that want to script up many tokens must call `POST /v1/tokens` directly via a service-account session (server-to-server), not via the CLI. Acceptable: token creation is rare; rotation is rarer; scripted provisioning belongs in a server-side script, not the CLI.
- **No "what tokens does my account own?" view in the CLI.** The web UI's tokens page is the single source of truth for server-side existence. The CLI knows what's *registered locally*.
- **Existing `nt token create` callers break.** Per the existing no-backcompat decision for the rewrite, accepted.
- **Internal-team env-switch requires re-init.** Switching `NO_TICKETS_ENV` between commands means the session-host mismatch warning fires until `nt init` runs again. Acceptable: env-switches are rare and the friction is one browser click.
- **Session-bearing path still requires `auth-server.ts` port** — ~250 LOC of local HTTP callback server, signal handlers, SIGINT race guards, port allocation. Not saved despite removing the destructive endpoints, because the session is still useful for identity reads.

### Mental model

| Concept | Before | After |
|---|---|---|
| Token types | session, push | session, push (unchanged) |
| Auth nouns | init, status, publish, validate, token, project | init, logout, status, publish, validate, token |
| Local config nouns | profile, project, token | project (as token registry key only) |
| URL knobs | `--profile`, `NO_TICKETS_API_URL`, `NO_TICKETS_AUTH_URL` | `NO_TICKETS_ENV` (preset), explicit URL pair (escape) |
| Server-side auth surfaces (bearer) | events, tokens.* | events, me, projects, event-types, GET tokens |
| Server-side auth surfaces (cookie/UI) | (none distinct) | POST tokens, DELETE tokens |

### Migration

Per the existing no-backcompat memo, no migration shim. Users on the current TS CLI move to the Rust binary via brew/scoop/cargo/install.sh, and their first action is:

1. (Optional) `nt init` for identity-aware commands.
2. Open the web UI, mint a fresh push token for their project.
3. Paste the `nt token add ...` one-liner shown by the UI.

Old `~/.notickets/credentials` and `~/.notickets/config.json` files: the new binary's credentials file format is compatible (same fields + new `host` tag). The config.json shape becomes incompatible because `profiles` disappears — the binary ignores any unknown top-level keys, so old `profiles` sections are silently preserved-but-unused on disk. A future cleanup utility (`nt config cleanup` or similar) could prune them; out of scope here.

## Affected work

- **`cross-platform-cli-binary` Task 5** (full Rust CLI port) — sub-tasks reshape per this ADR. Specifically: `init` ports unchanged; `status` reshapes per the new output; `publish` keeps push-token-only; `token add/list/remove` are new; `project link/list/unlink` and `token create/revoke` and `--profile` plumbing all drop from the port.
- **Separate fix doc** (to be filed alongside this ADR) — drive the actual implementation: Rust port changes, server endpoint moves, web UI snippet.
- **Server work (`no-tickets-service` repo)** — separate fix to move `POST /v1/tokens` and `DELETE /v1/tokens/{id}` out of the bearer-token auth class to UI cookie-session only. Add `GET /v1/me`, `GET /v1/projects` to the bearer-session class if not already present.
- **Web UI work** — emit the `nt token add ...` copy-paste snippet on token creation; ensure the create-token form is reachable.

## Alternatives considered

### Alt 1 — Delete `nt init` entirely; no session at all

Rejected. The session token has legitimate non-destructive uses (identity reads, project listing, future "logged in as X" experience). Removing it forces every identity-aware future command to either (a) not exist or (b) reintroduce the session machinery later. The security argument was specifically about destructive endpoints (`POST /v1/tokens`, `DELETE /v1/tokens/{id}`) — moving those out of bearer auth class addresses it without sacrificing identity reads.

### Alt 2 — Per-environment credential slots

Rejected. Adds a "which env am I logged into right now" concept to the credentials file shape. Better for the no-tickets team (multiple envs daily) but strictly worse for consumers (one env, ever — extra state they never use). The team is small; the cost of one extra `nt init` per env-switch is one browser click. **Option A wins on simplicity-vs-friction** for the dominant user.

### Alt 3 — Keep `nt project link` and the profile concept; just rename verbs

Rejected. Cosmetic change. Doesn't address frictions 2, 3, or 4. Doesn't simplify the Rust port. The right move is restructuring, not renaming.

### Alt 4 — Add session bearer alongside push token for ingest (`/v1/events`)

Rejected. Telemetry products universally use single-key ingest auth. Layering session + push doesn't raise security — an attacker reading the credentials directory gets both. Where session + bearer *does* help (financial, AWS STS, OAuth refresh) the data is sensitive, mutable, or cross-account reachable. Append-only event ingest does not fit that profile.

### Alt 5 — Keep `--profile` as a "power-user" flag

Rejected. The cost of an exposed concept is everywhere — help text, doc examples, flag parser, error messages. Benefit accrues to one consumer (no-tickets internal team) fully capable of using an env var. Asymmetric cost; remove.

### Alt 6 — Make `nt token list` show both local AND remote

Rejected. Two semantics under one verb. Either you implement `--remote` flag (then default is local — fine, but adds complexity), or "show both" (then the command has two failure modes and stale-session bugs reappear). Don't query the server from the CLI for this.
