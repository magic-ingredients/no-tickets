---
id: cross-platform-cli-binary
type: fix
title: Rust rewrite of CLI + MCP server, distributed as a single binary across all major platforms
phase: development
status: in_progress
severity: medium
created: 2026-05-09
updated: 2026-05-09
reported: 2026-05-09T00:00:00.000Z
resolved: null
---

# Fix: Rust rewrite of CLI + MCP, multi-channel binary distribution

## Phase context (4-phase client roadmap)

This fix covers **Phases 2 and 3** of a four-phase plan.

### Architectural commitment: no SDK in the heavy sense

The roadmap explicitly does **not** ship a multi-language transport-bearing SDK matrix. Three primitives carry the load:

1. **Zod schemas in `no-tickets-service`** — canonical event-type definitions, auto-published as language-native schemas packages (Zod / Pydantic / Go structs). Types and validation only; zero transport code.
2. **Rust binary (`nt`) — this fix.** Canonical transport. Distributed via every package manager. Handles auth, retries, error mapping, source merging, validation against bundled JSON Schemas. Supports `--stream` mode for warm in-process publishing from per-language wrappers.
3. **Per-language wrapper packages (~50–80 LOC each)** — pure spawn-glue over the binary. Hide stdio, parse structured errors, return typed Promises/results. They are *not* SDKs; they are syntactic sugar.

### Rationale

- **Phase 1 — fix the TS surface.** Need a working baseline for CLI publish, project registry, schema-distribution model, and a transitional TS scaffold for staging-test publishing before rewriting in Rust.
- **Phase 2/3 (this fix) — Rust binary, distributed everywhere.** CLI and MCP shouldn't require a Node runtime. *Everyone*, regardless of language, gets first-class CLI/MCP via a single binary distributed through every relevant package manager. The TS package this repo currently publishes is reshaped: same name, same `import { publish }` API, but its body becomes `execFile('nt', ...)` (or warm-stream variant). Consumers don't notice the swap.
- **Phase 4 — per-language wrappers on demand.** Each language gets a ~50-LOC wrapper + a schemas package (codegen-derived from Zod). Built when adoption justifies it.

### Phase-by-phase user journey by language

| Surface used | After Phase 1 | After Phase 3 (this fix) | After Phase 4 |
|---|---|---|---|
| **CLI** (`nt publish ...`) | TS-via-npm CLI | **Rust binary (every package manager)** | Rust binary (unchanged) |
| **MCP server** (agent tool calls) | TS server via npm | **Rust binary** | Rust binary (unchanged) |
| **In-code TS** (`import { publish }`) | Transitional TS scaffold | None — TS programmatic surface retired with no backcompat (no npm wrapper) | TS wrapper (~50–80 LOC, spawns binary in `--stream` mode) + Zod schemas package |
| **In-code Python** | None (raw HTTP or CLI) | None (raw HTTP or CLI) | Python wrapper (~50–80 LOC, `subprocess.Popen` streaming) + Pydantic schemas package |
| **In-code Go** | None (raw HTTP or CLI) | None (raw HTTP or CLI) | Go wrapper (~50–80 LOC, `exec.Cmd` streaming) + struct schemas package |

The "wrapper" pattern is identical across languages. Only the spawn primitive changes. All three call out to the same Rust binary against the same wire contract.

### Phase dependencies

| Phase | Fix | What lands | Depends on |
|---|---|---|---|
| **1** | `publish-shared-surfaces.md` | TS CLI `publish` wired (transitional scaffold); Zod schemas extracted to `@magic-ingredients/no-tickets-schemas`; project registry; flag shape | — |
| **2 (this fix)** | `cross-platform-cli-binary.md` | Full Rust rewrite of CLI + MCP, validating against the JSON Schema build artifact from `no-tickets-service`. **`--stream` mode** for persistent-subprocess wrappers. **Structured-error contract** on stderr. TS CLI and MCP scaffold retired. | Phase 1 (defines the surface to port) |
| **3 (this fix)** | same | Multi-channel distribution: cargo, Homebrew, Scoop, deb/rpm, install script. **No npm wrapper** — rewrite intentionally drops backcompat with the existing `@magic-ingredients/no-tickets` consumer surface. | Phase 2 |
| **4** | future fix | Per-language schemas packages (TS / Python / Go, codegen from Zod source via server-side pipeline) + per-language wrapper packages (~50–80 LOC each, including TS). | Phase 3 (stable binary + structured-error contract + `--stream` contract); also depends on server-side codegen pipeline |

## Issue Summary

**Reported:** 2026-05-09
**Severity:** medium

The TS CLI + MCP shipped via npm has structural ceilings:

- **Runtime dependency on Node.** Any CI runner / production host without Node can't run `nt publish`. Curl-piped `install.sh` of a single binary works in environments where `npm install` is policy-blocked or impossible (air-gapped, restricted runners).
- **Cold start.** Node startup + JS module loading lands in the ~200–400 ms range for the CLI, ~500 ms+ for MCP. Acceptable but not delightful for short-lived CLI invocations and adds up across thousands of CI events.
- **Memory footprint for MCP.** Long-running MCP servers carrying the V8 runtime sit at ~80–150 MB resident, multiplied by every IDE/agent process running them.
- **Distribution single-channel.** npm-only — no Homebrew, no Scoop, no system package managers, no curl-install. Means the CLI only reaches users already in the JS ecosystem.

A Rust rewrite produces a 5–15 MB static binary per target, sub-50 ms cold start, ~10–20 MB memory for MCP, and unlocks every standard distribution channel.

## Why Rust (not Bun-compile, not Go)

| Approach | Verdict | Reasoning |
|---|---|---|
| **Bun-compile of existing TS** | Rejected (was an earlier proposal) | 60–90 MB binary, ~300 ms cold start, embeds the Bun runtime. Faster to ship but locks in transitional binary size; we'd rewrite anyway eventually. Better to spend the engineering effort once. |
| **Go rewrite** | Considered | ~10 MB binary, fast startup, idiomatic for CLI distribution (gh, kubectl, terraform). Comparable outcome to Rust. Choice between Go and Rust is largely team/ecosystem preference. |
| **Rust rewrite (chosen)** | **Chosen** | 5–15 MB static binary, sub-50 ms startup, no GC pauses for MCP long-running processes, mature cross-compile story (`cross`, `cargo-zigbuild`), strong ecosystem for HTTP / JSON Schema / async (tokio + reqwest + jsonschema). Pairs naturally with `rmcp` for MCP server. |

Choosing Rust over Go is a deliberate call to favor: (a) tighter memory footprint and predictable latency for the MCP long-running case, (b) the type system catching protocol-level mistakes at compile time, (c) the toolchain (cargo) doubling as one distribution channel.

## What gets rewritten

**In scope of the Rust rewrite:**
- The CLI surface — `nt init`, `status`, `publish`, `project link/list/unlink`, `validate`, `connect`, `disconnect`, `token` subcommands, `version`, `help`
- The MCP server — discovery tools (list/describe), publish tool, source-from-transport, error mapping, registry cache, transport
- Project registry persistence (`~/.notickets/config.json` schema as defined in Phase 1)
- Browser-OAuth init flow (local HTTP listener, callback handler, the styled callback page from `untracked(cli): style CLI auth callback page`)

**Out of scope (stays TS, then becomes thin TS SDK in Phase 4):**
- The programmatic publish/transport library (`src/transport/`, `src/core/`, `src/registry/`)
- `validateEventLocally` as a JS-callable function (continues for TS SDK consumers)
- The Zod schemas in `no-tickets-service/packages/schemas/` (Rust binary consumes the JSON Schema build output, not the Zod source)

## Schema source of truth (carried over from Phase 1)

```
no-tickets-service repo
  packages/schemas/                   ── Zod source of truth (TS)
       │
       ├─► server runtime validation (TS)
       ├─► /v1/registry/event-types HTTP endpoint (Zod → JSON Schema, served with ETag)
       ├─► npm publish: @magic-ingredients/no-tickets-schemas
       └─► JSON Schema bundle (build artifact)
                  │
                  ▼
       ┌──────────────────────────────────────┐
       │ Rust CLI/MCP binary                  │
       │   - Embeds JSON Schema bundle at     │
       │     build time via include_bytes!    │
       │   - Validates with `jsonschema` crate│
       │   - Wire protocol = same HTTP API    │
       └──────────────────────────────────────┘
```

The Rust binary doesn't depend on the npm package at runtime. It depends on the JSON Schema build output (a small JSON file) which is published as part of the no-tickets-service release artifacts and pulled in via `cargo` build script. Schema versions are pinned per binary release.

## Distribution channels (Phase 3)

Single source artifact (per-target binary) shipped through every relevant channel:

| Channel | Audience | Mechanism |
|---|---|---|
| **GitHub Releases** | Direct download, `curl` install | Tagged release with per-target tarballs and checksums |
| **Install script** at `https://get.no-tickets.com` | Quickest curl-pipe install on Linux/macOS | Bash script: detects platform, downloads from GH Releases, verifies sha256 |
| **Homebrew tap** | Mac + Linux developers | `brew install magic-ingredients/tap/nt` |
| **Scoop bucket** | Windows developers | `scoop install nt` |
| **cargo install** | Rust ecosystem users | `cargo install nt-cli` (publishes to crates.io) |
| **deb / rpm** | Linux server installs | apt/yum repos hosted on GitHub Pages or a CDN |

No npm wrapper: the rewrite drops backcompat with the existing `@magic-ingredients/no-tickets` consumer surface (event-repository rewrite accepts the data-shape break). Existing npm users move to one of the channels above on next install; a TS wrapper for new programmatic use is deferred to Phase 4 (see phase table above).

## Compatibility audit (must verify before committing to Rust)

Specific items to confirm a Rust rewrite is feasible against current TS surface:

- [x] **MCP Rust crate** — **Resolved (2026-05).** `rmcp` is the official SDK at `modelcontextprotocol/rust-sdk`, 4.7M+ downloads on crates.io as of early 2026. Provides `#[tool]` macro, stdio transport, full tool/resource/prompt coverage. Task 2 spike still runs for round-trip confirmation, but the strategic risk is settled.
- [ ] **JSON Schema validation** — `jsonschema` crate (v0.46+) handles Draft 2020-12. Schema features the Zod source uses (refinements, custom `.refine()` predicates may not survive Zod → JSON Schema conversion cleanly) still need validation against a sampling of the existing event types.
- [ ] **OAuth callback flow** — `axum` for the localhost listener + callback route, **`oauth2` crate for the PKCE state machine / CSRF token / code-verifier exchange** (replaces hand-rolled protocol logic), `opener` crate for browser. Confirm 0.0.0.0 binding and timeout semantics match the TS implementation.
- [ ] **Cross-compile toolchain** — `cargo-zigbuild` (preferred for CI — no Docker-per-target) or `cross`. Both mature. Confirm CI matrix builds cleanly without per-platform runners (cost driver).
- [ ] **Auth file format** — keep `~/.notickets/credentials` and `~/.notickets/config.json` byte-compatible with the TS implementation so users can switch back if needed during rollout.
- [ ] **HTTPS client + Bearer auth + wire-contract** — `reqwest` with `rustls-tls` + `webpki-roots`, single POST to `/v1/events` with `Authorization: Bearer` header. Covered by Task 14 spike; clears the path for Task 4's full surface port.

## Public binary contract (consumed by per-language wrappers)

The Rust binary is the integration boundary for every consumer outside the Rust crate — CLI users, MCP clients, and per-language wrappers. Two parts of its surface are explicit deliverables of this fix because every wrapper across languages depends on them.

### Structured-error contract (stderr + exit codes)

The binary exits with a typed status code and writes structured JSON to stderr on failure. Wrappers in any language map exit code → typed exception by parsing stderr.

| Exit code | Class | stderr JSON shape |
|---|---|---|
| 0 | success | (none; stdout has the response) |
| 1 | `validation_error` | `{ error: "validation_error", typeId, batchIndex, issues: [{ path, message }] }` |
| 2 | `unknown_event_type` | `{ error: "unknown_event_type", typeId, suggestions: [string] }` |
| 3 | `permission_denied` | `{ error: "permission_denied", domain }` |
| 4 | `transport_error` | `{ error: "transport_error", message, retriable }` (after retries exhausted) |
| 5 | `not_authenticated` | `{ error: "not_authenticated", message }` |
| 6 | `project_not_registered` | `{ error: "project_not_registered", project, knownProjects: [string] }` |
| 7 | `usage` | `{ error: "usage", message }` (bad flags, missing required args) |
| 64+ | reserved | future error classes |

This shape MUST stay backward-compatible across binary releases. Wrappers compiled against an old version must continue to function against new binary versions (additive changes only — new exit codes, new fields, never renames or removals).

### `--stream` mode (persistent-subprocess publishing)

Per-event spawn cost is ~50 ms (fast, but compounds at high event rates). To support warm in-process publishing from per-language wrappers, the binary supports a stream mode:

```
nt publish --stream [--project DEFAULT] [--token-env-var X] [--url Y]
```

Behavior:
- **stdin**: one JSON object per line. Each line is `{ id, type, data, project?, subject?, occurredAt?, ... }` — id is a caller-chosen correlation token (any string).
- **stdout**: one JSON object per line. Each line is `{ id, ok: true, ingested, deduped, ids } | { id, ok: false, error: <typed-error> }` — id matches the request line.
- **stderr**: reserved for fatal binary-level errors (e.g., bad startup flags). Not used for per-event errors (those go on stdout with `ok: false`).
- **EOF on stdin**: binary drains in-flight requests, writes remaining responses, exits 0.
- **stdin closed mid-flight**: binary writes responses for completed requests, exits 0. In-progress requests get `ok: false, error: { error: "transport_aborted" }`.

Cost analysis:
- First call: ~50 ms (binary cold start)
- Subsequent calls: ~1 ms (pipe write + read)

This is the same pattern as `git cat-file --batch`, `clangd`, `aspell -a` — the well-trodden way for tools to handle warm state.

### Multi-project per stream

Each stream request line MAY override the default `--project` by including `project` in the JSON. The binary uses per-line override → flag default → `NO_TICKETS_TOKEN` env. Token resolution happens once per project per stream session (cached). This lets a single subprocess serve many projects from one parent — useful for CI orchestrators like tiny-brain.

### Wrapper expectations (informational)

Wrappers ship in this repo (TS) and in the language-specific repos (Python/Go) as part of Phase 4. Their job:

- Spawn-on-first-publish; reuse for subsequent calls
- Match request id to response by an internal map
- Handle subprocess crash by re-spawning transparently
- Kill subprocess on parent exit (POSIX process group inheritance handles this on most platforms; explicit `proc.kill()` on parent's exit handler as safety belt)
- Parse stderr for fatal errors and `ok: false` payloads on stdout for per-event errors
- Translate to typed exceptions in the caller's language

These behaviors are informational here; they live in the per-language wrapper packages, not in the binary.

## Runtime dependencies — design for zero

The binary should have effectively no runtime deps. Specific build choices to enforce that:

| Concern | Choice | Reason |
|---|---|---|
| **Linux libc** | musl target as default (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`) | Fully static. Runs in scratch/distroless/Alpine without glibc. Glibc target is a secondary artifact for users who specifically need it. |
| **TLS** | `rustls` (via `reqwest`'s `rustls-tls` feature), CA bundle via `webpki-roots` | No OpenSSL coupling, no system CA store dependency. Adds ~250 KB. |
| **HTTP client** | `reqwest` with `rustls-tls` feature, `default-features = false` | Avoids pulling native-tls and other heavy defaults. |
| **Async runtime** | `tokio` (already implied by `reqwest`/`axum`) | Pure Rust, statically linked. |
| **JSON Schema** | `jsonschema` crate, pure Rust | No C dependency. |
| **Browser opener** | `opener` crate (invokes `open`/`xdg-open`/`start` via `std::process::Command`) | Not a linked dep — uses OS shell facilities. Failure is non-fatal (URL is also printed for paste). |
| **macOS** | Default linkage to `libSystem.dylib` | Always present on macOS; not a real dep. |
| **Windows** | Default linkage to system DLLs; consider `+crt-static` for portability | Always present on Windows. |

### Per-platform runtime deps after these choices

| Platform | Real deps |
|---|---|
| Linux musl | None |
| Linux glibc (secondary artifact) | System glibc ≥ baseline (set by build environment) |
| macOS | None in practice (libSystem always present) |
| Windows | None in practice (system DLLs always present) |

Compare to today: the TS CLI's runtime deps are Node + `node_modules`. The Rust binary collapses that to ~10 MB self-contained.

## Off-the-shelf crate ecosystem (audited 2026-05)

A pass through crates.io confirms most of the bespoke surface has well-adopted Rust equivalents. The porting effort is smaller than a naive read of "rewrite in Rust" suggests — the custom code is the protocol body and command semantics, not the plumbing.

### Drop-in crates per concern

| Concern | Crate | Notes |
|---|---|---|
| **MCP server/client** | `rmcp` (official SDK) | `modelcontextprotocol/rust-sdk`. 4.7M+ downloads early 2026. `#[tool]` macro + stdio transport built in. Resolves this doc's previously-largest open question. |
| **CLI argument parsing** | `clap` (derive API) | Standard for production Rust CLIs (ripgrep, bat, gh). Subcommand derive removes parser boilerplate for `init`/`status`/`publish`/`project link/list/unlink`/`validate`/`connect`/`disconnect`/`token`. |
| **HTTP client + TLS** | `reqwest` + `rustls-tls` + `webpki-roots` | Already specified above. Pure-Rust TLS, no OpenSSL. |
| **Async runtime** | `tokio` | Required by `reqwest`, `axum`, `rmcp`. Statically linked. |
| **JSON Schema validation** | `jsonschema` (v0.46+) | Draft 2020-12. Build-once-validate-many pattern matches the per-batch publish flow. |
| **Local OAuth callback server** | `axum` + `oauth2` crate | `axum` for the localhost listener; `oauth2` crate handles the PKCE state machine, CSRF token, code-verifier exchange. Replaces hand-rolled protocol logic. |
| **Browser opener** | `opener` | Already in fix doc. |
| **JSON ser/de** | `serde` + `serde_json` | Wire protocol, config files, stream-mode JSONL. |
| **Error variants → typed exit codes** | `thiserror` (lib) + `anyhow` (app) | Maps directly to the structured-error contract: each variant derives an exit code + serializable stderr JSON shape. Task 26 becomes near-mechanical. |
| **Cross-platform config paths** | `directories` or `etcetera` | `~/.notickets/` resolution per OS conventions. |
| **OS keychain (optional upgrade)** | `keyring` | One-API access to macOS Keychain / Linux Secret Service / Windows Credential Manager. Optional improvement over the current plaintext `~/.notickets/credentials`. Out of scope for v1 of the rewrite; flagged for post-Phase-3 follow-up. |
| **Terminal output (colors, spinners)** | `owo-colors` + `indicatif` | Status output, auth callback polish. |
| **Logging (must go to stderr)** | `tracing` + `tracing-subscriber` | **Critical for MCP** — see gotcha below. |
| **Cross-compile toolchain** | `cargo-zigbuild` (preferred) or `cross` | `cargo-zigbuild` runs in CI without Docker-per-target overhead. |
| **Stream JSONL parsing** | `tokio::io::BufReader::lines()` + `serde_json` | Stdlib + serde, no extra crate. ~30 LOC for the request/response loop. |
| **CLI behavioral tests** | `assert_cmd` + `predicates` | Runs the binary, asserts stdout/stderr/exit code. Powers the feature-equivalence smoke matrix in Task 4. |
| **Release pipeline generator** | `cargo-dist` | **Chosen.** One config block in `Cargo.toml` generates: cross-compile CI matrix, GitHub Releases workflow, install script (`install.sh`), Homebrew formula auto-update, Scoop manifest auto-update. Collapses what would be four hand-rolled workflow tasks into one config block. See Task 6. Out of scope for `cargo-dist`: deb/rpm (Task 9) — stays hand-rolled. |
| **CLI self-update (`nt self-update`)** | `self_update` | Purpose-built for CLI tools — reads latest GH Release, downloads target binary, sha256-verifies, replaces self. ~20 LOC integration. Targets install.sh / direct-download users only (package-manager installs update via their package manager). **Not** Velopack — Velopack is GUI-app-shaped (Squirrel successor) and the wrong fit for a CLI/MCP binary. See Task 13. |

### Critical implementation gotchas

1. **MCP stdio purity (`rmcp`).** Any crate that logs to stdout — default `tracing-subscriber` config, a stray `println!`, a careless dependency — corrupts the JSON-RPC stream and causes Claude Code to silently disconnect. Standard fix: route all logging to stderr via `tracing_subscriber::fmt().with_writer(std::io::stderr)`. Add a CI assertion in `crates/nt-mcp/tests/` that the MCP binary writes only valid JSON-RPC frames to stdout under load (Task 2 acceptance criterion).

2. **`thiserror` for the structured-error contract.** Define a single enum `NtError` with `#[derive(thiserror::Error)]` variants per error class in the table above. One match arm maps each variant to its exit code + serialized stderr shape. Adding a new error class is one variant + one match arm — keeps the "additive-only" contract guarantee mechanical.

3. **`cargo-dist` opinionated defaults are acceptable.** Auto-generates Homebrew formula and install.sh assuming GitHub Releases hosting — matches plan. Auto-generates a single-binary release — matches. Divergent items (deb/rpm) are out of `cargo-dist` scope and stay hand-rolled as a separate task below.

### What's still genuinely custom

These have no off-the-shelf crate; they're code we write:

- The `--stream` JSONL protocol body (Task 27) — ~100 LOC of tokio stdio + serde.
- Each CLI subcommand's behavior matching the TS implementation (Task 4) — clap removes the parser boilerplate; the semantics still need porting one command at a time.
- The `build.rs` step that fetches the JSON Schema bundle from a `no-tickets-service` release artifact and pins by version (Task 3).
- Token resolution + project registry persistence (small layer over `serde_json` + `directories`).

## Test Plan

### Acceptance: feature-equivalence smoke matrix

Before retiring the TS CLI/MCP, run a parallel test:

- For each command (`init`, `status`, `publish`, `project link/list/unlink`, etc.): run TS implementation against staging, capture output. Run Rust implementation against same staging with same args. Diff outputs. Differences allowed only where called out in design (e.g., output formatting tweaks).
- For MCP: capture a session of tool calls from a real agent against the TS server. Replay against the Rust server. Same outputs / same JSON-RPC traces.

### CI smoke per target

| Target | Smoke: `nt --version` + mocked publish |
|---|---|
| Linux x64 (musl + glibc) | required |
| Linux arm64 | required |
| macOS x64 | required |
| macOS arm64 | required |
| Windows x64 | required |

## Tasks

### 1. Rust rewrite spike — single command end-to-end
status: completed
commitSha: f3b38bd

Build a Rust prototype of `nt status` against staging. Goal: validate the core toolchain (cargo, reqwest, serde, the project-config file read path) before committing to a full rewrite.

**Files to modify/create:**
- `crates/nt-cli/` (new) — Cargo workspace member
- `crates/nt-cli/src/main.rs` — `status` command only
- `crates/nt-cli/Cargo.toml`
- `Cargo.toml` (workspace root)

**Acceptance:**
- `cargo run -p nt-cli -- status --profile staging` matches TS output for the same auth state
- Documents any toolchain surprises in `docs/rust-spike-notes.md`

### 2. MCP server spike — `list_event_types` tool
status: completed
commitSha: 676ea22

Implement one MCP tool (`list_event_types`) using the official `rmcp` SDK with stdio transport and the `#[tool]` macro. Spawn it as an MCP server, drive it from a Claude Code MCP client. Confirm wire compatibility.

**Critical:** route all logging to stderr (`tracing_subscriber::fmt().with_writer(std::io::stderr)`). Anything to stdout corrupts the JSON-RPC stream and Claude Code silently disconnects.

**Files to modify/create:**
- `crates/nt-mcp/` (new)
- `crates/nt-mcp/src/main.rs`
- `crates/nt-mcp/src/tools/list_event_types.rs`
- `crates/nt-mcp/Cargo.toml`
- `crates/nt-mcp/tests/stdout-purity.rs` — asserts the binary writes only valid JSON-RPC frames to stdout under load (no log lines, no stray prints)

**Acceptance:**
- An IDE-loaded MCP client can list event types via the Rust MCP server
- Round-trip JSON-RPC traces match the TS server
- Stdout-purity test passes: under repeated tool invocation, every stdout byte is part of a valid JSON-RPC frame

### 3. JSON Schema bundle integration
status: completed
commitSha: ffa7402

Validate the local Rust-side toolchain (jsonschema crate + bundle loading + TS-parity validator API) against a **locally-vendored JSON Schema bundle** generated from the existing `@magic-ingredients/no-tickets-schemas` Zod source via `scripts/generate-schema-bundle.mjs`. The vendored bundle is the interim source; the canonical source becomes the release-artifact bundle produced by the sister fix.

**Cross-repo dependency (sister fix `client-roadmap-server-prerequisites` in `no-tickets-service`):**
- Sister Task 6 — JSON Schema build artifact (status: not_started)
- Sister Task 7 — Attach JSON Schema bundle to GitHub Releases (status: not_started)

Once those land, this fix gets a follow-up to swap `include_str!` for a `build.rs` that fetches+sha256-verifies the release-artifact bundle. The Rust validator API surface doesn't change between vendored and release-artifact modes — only the bundle source.

**Approach for the spike (this task):**
- Shared crate `crates/nt-schemas/` consumed by both nt-cli and nt-mcp (rather than duplicating per binary).
- Generator script `scripts/generate-schema-bundle.mjs` produces `crates/nt-schemas/schemas/event-types.bundle.json` from the npm `byTypeId` map. Uses each Zod schema's instance `.toJSONSchema()` method (not the top-level `z.toJSONSchema()` import) so cross-zod-instance type checks don't silently drop `.min()` / format / pattern constraints.
- `nt-schemas::validate(type_id, data)` returns `Option<Vec<ValidationIssue>>`: `None` for unknown type ids, `Some(vec![])` for valid, `Some(issues)` for invalid. Mirrors TS `validateEventLocally` shape (`{ path, message }`); paths are dot-joined for TS parity.

**Known divergence from server-side Zod (documented):**
- Zod `.refine()` predicates do NOT survive JSON Schema conversion. JSON Schema can't express arbitrary predicates. Server-side Zod validation still catches refine violations; local Rust validation is a strict subset of server validation, never a superset. Payloads that pass local but fail server are still server-rejected; no false-positive publishes.

**Files to modify/create:**
- `scripts/generate-schema-bundle.mjs` — Node generator (committed; re-run on schemas version bumps)
- `crates/nt-schemas/Cargo.toml` (new workspace member)
- `crates/nt-schemas/schemas/event-types.bundle.json` (generated, committed)
- `crates/nt-schemas/src/lib.rs` — `validate(type_id, data)` + `known_type_ids()` + `BUNDLE_VERSION`
- `crates/nt-schemas/tests/validate.rs` — bundle integrity + valid/invalid payloads + issue-shape parity
- workspace `Cargo.toml` — add nt-schemas member

**Follow-up task (created after sister Tasks 6+7 ship):**
- Swap `include_str!` for a `build.rs` that downloads the GH Release asset, verifies sha256, embeds via `include_bytes!`. API surface unchanged. ↳ now tracked as Task 3a below.

### 3a. Swap `include_str!` for `build.rs` fetch + sha256-verify of GH release bundle
status: completed
commitSha: de90a5d

Sister Tasks 6+7 shipped — the no-tickets-service repo now publishes versioned JSON Schema bundles to GitHub Releases with sha256 sidecars (first cut: [schemas-v0.2.1](https://github.com/magic-ingredients/no-tickets-service/releases/tag/schemas-v0.2.1)). Swap `crates/nt-schemas/src/lib.rs:31` from `include_str!` of a locally-generated, in-tree bundle to a `build.rs` that downloads the release asset, verifies its sha256, writes the bundle to `$OUT_DIR`, and re-exposes it via `include_str!(concat!(env!("OUT_DIR"), "/event-types.bundle.json"))`. Validator API surface stays identical.

**Versioning:**
- Schemas version is **independent of nt-cli / nt-mcp version**. Pin it in `crates/nt-schemas/Cargo.toml` under `[package.metadata.no-tickets-schemas] version = "0.2.1"` (or as a const `SCHEMAS_VERSION` in `build.rs`). Bumping schemas is a one-line change.
- The pinned version flows into both the download URL (`releases/download/schemas-v{VERSION}/...`) and the `BUNDLE_VERSION` assertion in `tests/validate.rs`, so a version-bump that mismatches the published asset fails compile / fails the integrity test.

**Offline-build policy:** none. Per discussion: builds only happen in GH Actions (network available) and on developer machines (where missing network already blocks dev work). `build.rs` fails fast with a clear error if the asset is missing, the sha256 doesn't match, or the network is unavailable. Documented in `docs/rust-spike-notes.md`.

**`cargo install nt` consideration:** this means downstream `cargo install nt` invocations also require network to `github.com/magic-ingredients/no-tickets-service` at build time. Acceptable trade-off — `cargo install` already requires network to crates.io; the additional release-asset fetch is a single HTTP round-trip with a clear failure mode.

**Retire the local generator:** `scripts/generate-schema-bundle.mjs` and `crates/nt-schemas/schemas/event-types.bundle.json` are no longer the source of truth — delete both once the build.rs path is green. The vendored bundle line in Task 3 was a spike artefact; the canonical source is the release asset from this task forward.

**Files to modify/create:**
- `crates/nt-schemas/build.rs` (new) — minimal HTTP fetch (use `ureq` or `reqwest::blocking` as a `[build-dependencies]` crate), sha256 verify (`sha2`), write to `$OUT_DIR/event-types.bundle.json`
- `crates/nt-schemas/Cargo.toml` — add `[package.metadata.no-tickets-schemas] version = "0.2.1"`; add `[build-dependencies]` for the HTTP + sha256 crates
- `crates/nt-schemas/src/lib.rs:31` — swap `include_str!("../schemas/event-types.bundle.json")` for `include_str!(concat!(env!("OUT_DIR"), "/event-types.bundle.json"))`
- `crates/nt-schemas/schemas/event-types.bundle.json` — **delete** (no longer vendored)
- `crates/nt-schemas/tests/validate.rs` — assert `bundle_version()` matches the pinned metadata version
- `scripts/generate-schema-bundle.mjs` — **delete**
- `package.json` — remove the generator script reference if any
- `docs/rust-spike-notes.md` — append Task 3a notes (fetch URL pattern, sha256 verify approach, offline-build trade-off)

**Acceptance:**
- Clean `cargo build` downloads `schemas-v{pinned}` bundle + `.sha256`, verifies, embeds. No vendored bundle in the source tree.
- Bumping `[package.metadata.no-tickets-schemas].version` in Cargo.toml is the only change needed to track a new schemas release.
- Sha256 mismatch (simulated by editing the pinned hash, or by stale cache) fails the build with a clear error.
- `nt-schemas`'s public API (`validate`, `known_type_ids`, `bundle_version`) is byte-for-byte unchanged; all existing `tests/validate.rs` cases pass.

### 4. Full CLI surface port
status: completed
commitSha: fc8175b

Port all commands to Rust per the ADR-0002 surface. Use `clap` with the
derive API for subcommand parsing. Match flag parsing, error messages,
exit codes, JSON output schemas.

The original scope listed `project link/list/unlink` and
`connect`/`disconnect` — those were superseded by the simpler
`token add/list/remove` + browser-based `init` flow before v0.1.0 and
never shipped. The shipped surface (final, v0.1.1) is: `init`, `logout`,
`status`, `publish`, `validate`, `self-update`, `token add/list/remove`.
The MCP server (`no-tickets-mcp`) is the second binary; its tools are
`list_event_types`, `publish_event`, `describe_event_type`.

Also retroactively scoped out: `--subject-type` / `--subject-id` flags
on `publish` (and the MCP `subject` arg). These were implemented and
shipped in v0.1.0, then removed in v0.1.1 because subjects aren't
modelled server-side; the help-audit follow-up commit cleaned them up.

**Slice progress (multi-cycle; this task aggregates several TDD cycles):**
- `init`, `logout`, `status`, `token add/list/remove` — landed via the side-fix `implement-adr-0002-cli-surface`
- `publish` (single-event, spike-scope) — landed via Task 14
- `validate` — landed at fc8175b (this fix, TDD cycle 1 of Task 4)
- `publish` optional metadata (`--source-name`, `--source-attributes`, `--parent`, `--trace`, `--dedupe-key`) — landed via Task 15
- `publish` batch mode (`--file` / stdin) — landed via Task 16
- `publish` retry/backoff on transient errors — landed via Task 17
- `publish` source auto-detection / merging — landed via Task 18

**Files to modify/create:**
- `crates/nt-cli/src/commands/`
- `crates/nt-cli/tests/cli-equivalence.rs` — uses `assert_cmd` + `predicates` to run the binary against staging and assert stdout/stderr/exit code per command

**Acceptance:**
- Feature-equivalence smoke matrix (above) passes for every command

### 26. Structured error contract on stderr + exit codes
status: completed
commitSha: 2bc103b

Implement the structured-error contract documented in "Public binary contract" above. Every failure case maps to a typed exit code with a single-line JSON object on stderr. This is the contract per-language wrappers parse; backward compatibility is mandatory.

Use `thiserror` for the error enum and a single match arm to map variant → exit code + serialized stderr JSON. Adding a new error class is one variant + one match arm — keeps the additive-only guarantee mechanical.

**Files to modify/create:**
- `crates/nt-cli/src/error.rs` — typed error variants (`thiserror`-derived) + serialization
- `crates/nt-cli/tests/structured-errors.rs` — exit-code + stderr-shape assertions for each error class (via `assert_cmd`)
- `docs/binary-error-contract.md` — public contract doc consumers can rely on

**Acceptance:**
- Each error class produces the documented exit code + stderr JSON shape
- Adding a new error class is purely additive (new exit code; old ones unchanged)
- Contract doc lives at a stable URL referenced by per-language wrappers

### 27. `--stream` mode for warm in-process publishing
status: not_started

Implement the streaming protocol documented in "Public binary contract": JSONL on stdin → JSONL on stdout, id-correlated, multi-project per session, graceful EOF.

This is what per-language wrappers use to keep the binary alive across many publish calls (~1 ms per event after first spawn vs ~50 ms cold). Same pattern as `git cat-file --batch`, `clangd`, `aspell -a`.

**Files to modify/create:**
- `crates/nt-cli/src/commands/publish_stream.rs`
- `crates/nt-cli/tests/stream-mode.rs` — assertions on:
  - Request/response id correlation
  - Multi-project per stream (per-line `project` overrides flag default)
  - EOF drains in-flight + exits 0
  - Stdin-closed-mid-flight produces `ok: false, transport_aborted` for in-progress
  - Backpressure (large request, slow consumer): no deadlock
- `docs/binary-stream-protocol.md` — public protocol doc

**Acceptance:**
- Ten thousand events streamed through one subprocess in <2 s end-to-end (bounded by network + server, not binary overhead)
- Per-event overhead measured at <2 ms median on the wrapper side
- Crash recovery: if the binary panics mid-stream, in-flight responses surface as `ok: false, transport_aborted`; wrapper can re-spawn cleanly

### 28. Consolidate nt-cli and nt-mcp into a single shippable cargo package
status: completed
commitSha: 241025c
depends_on: [6]

The Task 6 scaffold (commits da19515, 76bc57e) configured cargo-dist but `dist plan` produces TWO releases because `nt-cli` and `nt-mcp` are separate cargo packages each declaring a `[[bin]]`. Two installers (`nt-cli-installer.sh`, `nt-mcp-installer.sh`), two homebrew formulae (`nt-cli.rb`, `nt-mcp.rb`), two tarballs per target. Bad UX: a user installing "no-tickets" has to know there's a CLI and a separate MCP server packaged independently and run two installers.

Task 6's acceptance criterion `brew install magic-ingredients/tap/nt` requires both binaries to ship in ONE release under the formula name `nt`. This task does the package restructure that enables that.

**Approach:**
- Cargo package was renamed `nt-cli` → `no-tickets` (matches the product name and the GitHub repo / homepage / domain — every other channel already says `no-tickets`). The daily-use binary stays `nt` (short, type-friendly). Single product name across all install channels avoids a "wait, is it nt or no-tickets?" moment in the docs (`cargo install no-tickets`, `brew install .../no-tickets`, `curl … no-tickets-installer.sh`, tarballs `no-tickets-{target}.tar.xz`, formula `no-tickets.rb`).
- The `nt` name on crates.io is taken by an unrelated NetworkTables crate, which forced the cargo-side decision; we chose to align everything rather than diverge only on crates.io.
- Convert `nt-mcp` to lib-only:
  - Create `crates/nt-mcp/src/lib.rs` re-exporting modules and a `pub async fn run() -> anyhow::Result<()>` containing the current `main()` body (tokio runtime stays in the consumer)
  - Delete `crates/nt-mcp/src/main.rs`
  - Remove `[[bin]]` from `crates/nt-mcp/Cargo.toml`
- Add the `nt-mcp` binary as a second `[[bin]]` of the `nt` package:
  - `crates/nt-cli/src/bin/nt-mcp.rs` — thin entry: `#[tokio::main(current_thread)] async fn main() -> anyhow::Result<()> { nt_mcp::run().await }`
  - `nt-cli/Cargo.toml` gains `nt-mcp = { path = "../nt-mcp" }` plus `anyhow` (if not already present) and a matching `[[bin]]` entry
- Move `crates/nt-mcp/tests/mcp{,.rs,/*.rs}` to `crates/nt-cli/tests/mcp{,.rs,/*.rs}` — they use `env!("CARGO_BIN_EXE_nt-mcp")` which only resolves in the same package as the binary

**Files to modify/create:**
- `crates/nt-cli/Cargo.toml` — add `nt-mcp` + `anyhow` deps, add `[[bin]] nt-mcp`
- `crates/nt-cli/src/bin/nt-mcp.rs` (new) — thin entry calling `nt_mcp::run()`
- `crates/nt-mcp/Cargo.toml` — drop `[[bin]]`
- `crates/nt-mcp/src/lib.rs` (new) — module re-exports + `pub async fn run()`
- `crates/nt-mcp/src/main.rs` (delete)
- `crates/nt-mcp/tests/**` → `crates/nt-cli/tests/mcp{,.rs,/*.rs}` (move)

**Acceptance:**
- `cargo check --workspace` and `cargo test --workspace` pass — no behavioural regression
- `dist plan` announces ONE release (the `nt-cli` package) containing both `nt` and `nt-mcp` binaries across the five targets
- Generated formula is `no-tickets.rb`, installer is `no-tickets-installer.sh`, tarball is `no-tickets-{target}.{tar.xz,zip}` — single product-name shape across every channel; binaries inside are `nt` and `nt-mcp`
- `cargo run -p nt-cli --bin nt` and `cargo run -p nt-cli --bin nt-mcp` both work (dev workflow preserved)

### 29. Smoke-test release pipeline with a prerelease tag
status: completed
commitSha: b27d2ed
depends_on: [6, 10, 28]

End-to-end validation of the assembled distribution pipeline before committing to `v0.1.0`. Push `v0.0.1-prerelease.1` (cargo-dist gates `publish-homebrew-formula` on `!is_prerelease`, so the tap stays untouched on smoke tests — exactly what we want).

**Steps:**
1. `git tag v0.0.1-prerelease.1 && git push origin v0.0.1-prerelease.1`
2. Watch Actions tab; expect five `build-local-artifacts` matrix jobs + `host` + `announce` green, and `publish-homebrew-formula` skipped (not failed).
3. Verify `curl -fsSL https://get.no-tickets.com | sh` returns the install script (validates Task 10's worker + Task 6's installer naming end-to-end).
4. Run the install command; expect `nt --version` to print `0.0.1-prerelease.1` and `nt-mcp` to exist alongside.
5. If anything's wrong, delete the tag locally + remotely + delete the GH Release, fix, retry.

**Acceptance:**
- GitHub Release for the prerelease tag contains 5 tarballs + shell + powershell installer + sha256 checksums
- `curl -fsSL https://get.no-tickets.com | sh` installs working `nt` and `nt-mcp` to `~/.local/bin/`
- `publish-homebrew-formula` job shows "skipped" status (not failed) — proves cargo-dist's prerelease gating works as expected

**Resolution note (2026-05-20):** Smoke-test went straight to `v0.1.0` rather than the spec'd prerelease tag — judgment call after two failed iterations on permissions / token name (the prerelease ceremony was deemed overcautious once the residual risk was just build matrix + homebrew publish). Acceptance criteria 1 and 2 are met (5-target archives + sha256 sidecars + both installers present at v0.1.0; `curl get.no-tickets.com | sh` returns 200 text/x-shellscript). Criterion 3 (publish-homebrew-formula skipping on prerelease) was not exercised — `publish-homebrew-formula` ran and succeeded against v0.1.0, which proves the job works end-to-end but not the prerelease-skip gating. Subsequent prerelease cuts would validate that gating cheaply if needed.

Failures encountered + their fixes (all on the path to green v0.1.0):
- `actions/upload-artifact` 403 on FinalizeArtifact — top-level `permissions: { contents: write }` zeroed `actions:` scope. Fixed by job-level `actions: write` on plan / build-local-artifacts / build-global-artifacts / host (commits 9567792, 2151e4b).
- Tag-vs-Cargo-version mismatch — `v0.1.0-prerelease.1` didn't match the `0.1.0` crate versions, so `dist host` errored "This workspace doesn't have anything for dist to Release!" Switched to `v0.1.0` direct.
- nt-schemas build.rs 401 — `gh release download` against the private `magic-ingredients/no-tickets-service` repo failed under the workflow's repo-scoped `GITHUB_TOKEN`. Fixed by injecting a fine-grained PAT (Contents:Read on no-tickets-service) as `GH_TOKEN` at job-level on build-local-artifacts + build-global-artifacts (commits 96a26a7, f42826e, b27d2ed). Stored as repo secret `SCHEMAS_READ_TOKEN`.
- `get.no-tickets.com` continued to 502 after v0.1.0 went green — the repo itself was still private, so the release-asset URLs 404'd unauthenticated. Resolved by flipping `magic-ingredients/no-tickets` to public (intended state per Task 31).

Verified post-resolution:
- `curl -sI https://get.no-tickets.com` → `HTTP/2 200 text/x-shellscript`
- `gh release view v0.1.0 --json assets` lists all 17 expected artifacts
- `publish-homebrew-formula` job green; `magic-ingredients/homebrew-tap` carries `Formula/no-tickets.rb`

### 30. Rename source.name wire identifier `"nt-cli"` → `"no-tickets"`
status: completed
commitSha: 8cc2aaa

Every event the CLI publishes carries `"source": { "name": "nt-cli" }` — a vestige of the original package name. Memory `[[project_no_v1_backcompat]]` permits wire-format changes; renaming to `"no-tickets"` aligns with the cargo package + product name. Binary name `nt` stays the daily-use identifier; source.name is the product-facing wire identifier.

**Files to modify:**
- `crates/nt-cli/src/commands/publish.rs` — `DEFAULT_SOURCE_NAME` constant
- `crates/nt-cli/src/commands/publish/envelope.rs` — literals + test assertions
- `crates/nt-cli/src/commands/publish/post.rs` — test assertion
- `crates/nt-cli/src/commands/publish_batch/source.rs` — many literals + tests
- `crates/nt-cli/src/commands/publish_batch.rs` — doc comment
- `crates/nt-cli/src/main.rs` — clap help text for `--source-name` override
- `crates/nt-cli/tests/publish/happy_path.rs` — assertions
- `crates/nt-cli/tests/publish/batch.rs` — assertions + comment
- `crates/nt-cli/tests/mcp/publish_event.rs` — comments mentioning the default
- `crates/nt-mcp/src/server.rs` — comment reference

**Acceptance:**
- Every emitter and assertion uses `"no-tickets"`
- `cargo test --workspace` clean

### 31. Public-repo polish files
status: completed
commitSha: 670ca49

Repo is going public. Add the GitHub-recognized OSS files so the repo page surfaces the right buttons.

**Files to create:**
- `SECURITY.md` — vulnerability reporting policy (e.g. `security@no-tickets.com` or "open a private security advisory on GitHub")
- `CONTRIBUTING.md` — optional; only if external PRs are wanted

The existing fix docs, AGENTS.md, and `.tiny-brain/` material are already public-safe; this task is only the missing OSS conventions, not a broader audit.

**Acceptance:**
- `SECURITY.md` present in repo root
- GitHub repo page shows the "Report a vulnerability" button under the Security tab

### 32. Widen pre-commit fmt scope to whole workspace
status: completed
commitSha: 8c04c4d

Root cause of the recurring fmt drift that's been showing up as "incidental rustfmt cleanup" in Task 28-era commits. `package.json` scripts run `cargo fmt --check -p no-tickets` which only touches the main package; the other crates (`nt-mcp`, `nt-core`, `nt-schemas`) drift silently.

**Files to modify:**
- `package.json` — `rust:fmt`, `rust:fmt:fix`, possibly `rust:clippy` / `rust:check` drop the `-p no-tickets` scope so the whole workspace is checked

**Acceptance:**
- A deliberately misformatted file in `crates/nt-core/` is caught by the pre-commit hook
- No drift-by-accumulation across crates in subsequent commits

### 33. Decide TS-SDK Phase 4 survival
status: not_started

Parked architectural question. Phase 4 (per-language wrappers) currently lists TS alongside Python and Go: a ~50–80 LOC wrapper that spawns `nt` (potentially in `--stream` mode for warm reuse). Open question: does the npm package come back, or is the Rust binary the only client surface forever and TS users go through `execFile('nt', ...)` themselves?

Decision blocks nothing in Phase 3 but sets the Phase 4 scope and tells Task 13 (docs) whether to mention `npm install @magic-ingredients/no-tickets` at all.

**Acceptance:**
- A short ADR or a paragraph in `docs/rust-spike-notes.md` captures the decision and its rationale, so future Phase 4 work doesn't have to re-litigate it.

### 34. Scoop manifest support (Windows)
status: superseded
commitSha: null

**Superseded — premature for v0.1.0 and possibly indefinitely.** Two-audience analysis (session 2026-05-15): CI runners install via curl regardless of platform; only developer workstations care about package managers. Scoop targets workstation-Windows users only, and Windows is one of three PM ecosystems (Scoop, winget, Chocolatey) — Scoop is the smallest-reach of the three (winget ships with Windows 10/11 by default, Chocolatey has wider enterprise penetration, Scoop requires the user to bootstrap Scoop itself first). Optimising for the smallest Windows-workstation slice before there's any Windows demand signal isn't worth the maintenance cost.

The PowerShell installer + direct ZIP download cover day-one Windows users. If Windows demand surfaces and a PM channel is asked for, `winget` is the strategic pick (Microsoft first-party, no bucket-repo bootstrap, manifests committed via PR to `microsoft/winget-pkgs`). Open a new task at that point.

### 35. CI integration polish
status: completed
commitSha: 3ac57f1
depends_on: [13, 29]

Two-audience framing surfaced during session 2026-05-15: CI runners use `curl … | sh` regardless of platform (no PM, no state between runs); only developer workstations install via brew/cargo/PowerShell/etc. CI usage is likely the larger volume once adoption picks up — every PR / push pulls the binary, vs. one install per dev workstation. This task closes the gap on CI-side ergonomics.

**`docs/install.md` additions:**
- A new "Using no-tickets in CI" section with copy-pasteable recipes for the common providers:
  - GitHub Actions — single install step + the `echo "$HOME/.local/bin" >> $GITHUB_PATH` step that's required because the shell installer modifies rc files but each Actions step is a fresh shell
  - GitLab CI — `before_script` install + PATH
  - Generic shell (CircleCI, Bitbucket Pipelines, Jenkins, Drone) — equivalent recipe
- A short note on CI auth: `NO_TICKETS_TOKEN` env var bypasses the interactive `nt init` flow; tokens are minted via `nt token add` on a workstation and stashed in the CI provider's secret store.

**Optional second deliverable: `magic-ingredients/install-no-tickets@v1` GitHub Action**

A composite Action that wraps `curl … | sh` + the PATH addition + a `nt --version` smoke check into one `uses:` line. Marketplace-published so GH-Actions users can do:
```yaml
- uses: magic-ingredients/install-no-tickets@v1
- run: nt publish ...
  env:
    NO_TICKETS_TOKEN: ${{ secrets.NO_TICKETS_TOKEN }}
```
instead of the 3-step recipe. Optional because the recipe is what most CI providers can use; the Action is the gold-plating for GH-Actions users specifically.

**Smoke-test prerequisite:** the GH Actions recipe should be verified against a real workflow run — fits naturally into Task 29's smoke-test of `v0.0.1-prerelease.1` (add a "verify CI install" step to that task's checklist).

**Acceptance:**
- `docs/install.md` gains a verified "Using no-tickets in CI" section with at least the GH Actions recipe.
- (Optional) `magic-ingredients/install-no-tickets@v1` published on the GH Marketplace, with README pointing at it as the preferred GH-Actions install path.

**Resolution note (2026-05-20):** `docs/install.md` now has a "Using no-tickets in CI" section with three recipes: GitHub Actions (with the mandatory `$GITHUB_PATH` export), GitLab CI (with reusable YAML anchor), and a generic-shell recipe covering CircleCI / Bitbucket Pipelines / Jenkins / Drone. CI-auth note (NO_TICKETS_TOKEN env-var bypass of interactive init) included. The optional `magic-ingredients/install-no-tickets@v1` GitHub Marketplace Action is deferred — needs a separate repo, Marketplace publishing flow, and a real GH-Actions demand signal before the maintenance cost is worth it. Recipe is what most CI providers will use.

### 5. Full MCP server surface port
status: completed
commitSha: 0410215

Port all tools and discovery flow to the Rust MCP server. Match the canonical tool descriptors, input schemas, response shapes.

**Resolution note (2026-05-20):** Aggregator for sub-tasks 19-24, all closed. `publish_event` (19, ce52ef0), `describe_event_type` (20, c53bc52), and real-server `list_event_types` (23) shipped; `nt-core` extraction (24, 0410215) was the last sub-task. Tasks 21 (`run_interaction`) and 22 (`create_subject`) superseded mid-flight per `project_workflow_by_events` + `project_no_subjects_in_model`. commitSha pinned to 24's final commit — the chronological close of the full surface.



**Scope clarification (2026-05-14):** the TS `create-server.ts` wires only `validate` (legacy `.notickets/` directory checker) and `status` into the actually-exposed MCP surface. The richer toolset (`list_event_types`, `describe_event_type`, `publish_event`, `run_interaction`, `create_subject`) lives in `src/mcp/tools/handlers.ts` but was never registered. The fix doc explicitly names list/describe/publish as in-scope, so the Rust port targets the planned-but-unwired TS surface, not the vestigial validate/status pair.

**Scope revision (2026-05-15):** `run_interaction` AND `create_subject` superseded (see Tasks 21 and 22). Workflows in no-tickets are modelled as event sequences sharing a run_id, with autonomous workers emitting their own events; subjects exist as inert wire-envelope storage but no production subject types are registered server-side. MCP surface narrowed to **three tools**: `list_event_types`, `describe_event_type`, `publish_event`. The `subject: { type, id }` field on the event envelope is retained on `publish_event` as a forward-compat slot matching the server envelope.

This task is split into sub-tasks 19–24 below (integer suffixes match the Task 4 → 14-18 split convention recognised by the task-sync tool); the Task 5 top-level closes once 19–24 all complete. `list_event_types` against local fixtures already exists from the Task 2 spike (commit 676ea22).

**Files to modify/create (across sub-tasks):**
- `crates/nt-mcp/src/tools/<name>.rs` per tool
- `crates/nt-mcp/src/server.rs` to register each tool
- `crates/nt-core/` (new workspace member) — shared HTTP/auth/urls infra between nt-cli and nt-mcp, extracted when the second consumer (nt-mcp publish path) lands. Whether to extract eagerly or duplicate then extract is decided per-sub-task.

**Acceptance (top-level):**
- Real agent driving the Rust MCP server produces the same tool outputs as the TS server for the same inputs (per sub-task acceptance below).

### 19. `publish_event` MCP tool — local schema validate + HTTPS POST
status: completed
commitSha: ce52ef0

Implement the MCP `publish_event` tool against the canonical TS reference at `src/mcp/tools/handlers.ts::handlePublishEvent`. Single-event publish from the MCP server: local schema validation (bundled JSON Schema via `nt-schemas::validate`) gates the call before any HTTP, then POST to `/v1/events` with the resolved auth.

**Auth model (MCP):** unlike `nt-cli`, the MCP server is spawned by its client (Claude Code etc.) with env vars from the client's `mcp.json`. Auth resolution is env-var-only:
- `NO_TICKETS_TOKEN` (required)
- `NO_TICKETS_API_URL` / `NO_TICKETS_ENV` / `NO_TICKETS_AUTH_URL` per existing url-resolver semantics
- No credentials-file fallback, no project registry lookup, no interactive browser flow

This is single-token / single-project per server invocation. Multi-project routing (via the registry) is deferred to a future MCP-side enhancement.

**HTTP/auth code provenance:** for this first sub-task, duplicate the minimum HTTP + auth bits from `nt-cli` into `nt-mcp` rather than extracting `nt-core`. The duplication is small (~100 LOC: a simplified `Client` over `reqwest`, env-var token read, url resolution copy). When Task 20 / 21 lands and a second consumer of the duplicated code appears, extract to a shared `nt-core` crate via Task 24.

**Tool args (matches TS):**
- `project: string` — used for `source.attributes.project` on the wire; does NOT route to a different token
- `type: string` — event type id
- `data: object` — event payload
- `subject?: { type, id }` — optional
- `occurred_at?: string` — optional
- `parent_event_id?: string` — optional
- `trace_id?: string` — optional
- `dedupe_key?: string` — optional

**Tool result (matches TS):**
- `id: string` — the assigned event id from the server response
- `deduped: boolean` — true iff server reported `{ ingested: 0, deduped > 0 }`

**Source identity:** every event the MCP server publishes carries `source.name = "nt-mcp"` (mirrors TS `source: deps.source` which is fixed at server-creation time). The machine-hash attribute (Task 18 env var `NO_TICKETS_INCLUDE_MACHINE`) applies the same way as `nt publish`.

**Files to modify/create:**
- `crates/nt-mcp/src/tools/publish_event.rs` (new)
- `crates/nt-mcp/src/server.rs` — register tool with `#[tool]` macro
- `crates/nt-mcp/src/transport.rs` (new, duplicated) — minimal reqwest-backed Client
- `crates/nt-mcp/src/auth.rs` (new, simplified) — env-var-only token read
- `crates/nt-mcp/src/urls.rs` (new, copied/simplified) — env-var URL resolution
- `crates/nt-mcp/Cargo.toml` — add reqwest, sha2/hostname for machine hash, wiremock as dev-dep
- `crates/nt-mcp/tests/publish_event_tool.rs` — wiremock-driven tool integration test

**Acceptance:**
- Valid args → POST to `/v1/events` with the correct envelope shape and Bearer auth → tool result has the server-returned event id
- Invalid args (unknown type, schema fail) short-circuit before any HTTP call, surface a structured MCP error
- Missing `NO_TICKETS_TOKEN` surfaces a not-authenticated error to the MCP client before any HTTP call
- 5xx response after retry exhaustion surfaces a transport error
- 4xx response surfaces a domain-specific error (permission denied for 403, validation for 422, etc.)
- Wiremock test covers the happy path, schema-fail short-circuit, 401, 403, 422, 5xx-retry-exhausted

### 20. `describe_event_type` MCP tool — server GET + schema synthesis
status: completed
commitSha: c53bc52

GET `/v1/registry/event-types/{id}` and return the JSON Schema plus a synthesised example payload (`example-synth.ts` equivalent in Rust). Smaller than Task 19 — single GET, response transformation. Naturally pairs with Task 23 (cache).

**Files to modify/create:**
- `crates/nt-mcp/src/tools/describe_event_type.rs` (new)
- Example synthesis port (port `src/lib/example-synth.ts` to Rust)

### 21. `run_interaction` MCP tool
status: superseded
commitSha: null

**Superseded 2026-05-15.** Dropped from the MCP surface. The original
plan modelled compound actions as a synchronous server-side handler
that emits N events and returns the events list. That conflicts with
how workflows actually work in this system:

  - main agent emits `workflow.run.started.v1` (optional)
  - workers do their tasks, each emitting their own events
    (parent_event_id / trace_id threaded from the run id)
  - main agent emits `workflow.run.completed.v1` (or `.errored.v1`)

The server stitches the workflow by correlation key; there is no
coordinator. `run_interaction` would have imposed a second, parallel
pattern ("if you want a workflow, wrap in an interaction") competing
with the established event-threaded model. Workers are autonomous;
the calling agent never needs the downstream events back in a
synchronous response.

The Task 21 RED + GREEN + REFACTOR + MUTATION commits
(3c4ed53, 5568fa5, 967e281) are kept in history for the design
exploration; the tool registration, body, and integration tests
were removed in a follow-up `chore(mcp): supersede Task 21` commit.

If a synchronous server-side compound write is ever needed as an
escape hatch, it should be designed afresh — not resurrected from
this task.

### 22. `create_subject` MCP tool
status: superseded
commitSha: null

**Superseded 2026-05-15.** Dropped from the MCP surface. Audit of the
sister `no-tickets-service` repo on 2026-05-15 confirmed:

  - Zero production calls to `registerSubjectType(...)` — the function
    is only used in `.test.ts` files. The server-side infrastructure
    (registry, reducer, replay, route handlers, db tables) exists as
    scaffolding but no production subject types are registered.
  - The reducer pipeline (`server/subjects/apply-reducer.ts`) short-
    circuits on missing registrations: `const spec = getSubjectType
    (event.subjectType); if (!spec) return;`. DB columns get filled,
    no state is materialised.
  - `POST /v1/subjects` is unusable in practice: it dispatches to
    `getSubjectType` and returns nothing meaningful without
    registered types.

The current model is **projects + event-types-per-domain** (11 event
types live in `byTypeId`: 3 `ai.*` + 8 `product.*`). Subjects are a
planned-but-unwired concept on the server side too — not just in the
TS MCP surface that was never registered in `create-server.ts`.

The `subject: { type, id }` field on the event envelope is retained
on `publish_event` (Task 19) as a forward-compat slot — the server's
`eventEnvelopeSchema` still accepts it and `envelopeToRawEvent`
stores it in `subjectType` / `subjectId` columns. Today it's inert
denormalised storage; if subject types are ever registered, the
field is wired. Stripping the field would diverge from the live
server envelope for no benefit.

If a `create_subject`-shaped tool is ever needed, it should be
designed against an actually-registered subject type, not
resurrected from this task.

### 23. Real-server `list_event_types` (replace fixtures)
status: completed
commitSha: f7a0aa5

Switch `list_event_types` from local fixtures (Task 2 spike) to a real `GET /v1/registry/event-types` call with an in-memory cache + async refresh. Closes out the spike.

**Completed 2026-05-15.** `crates/nt-mcp/src/registry_cache.rs`
(new) wraps `Arc<RwLock<Option<Arc<Vec<EventTypeSpec>>>>>` with a
cold-fetch-on-first-call + warm-spawned-refresh pattern. Refresh is
throttled by `NT_REGISTRY_REFRESH_INTERVAL_MS` (default 5s) so a
busy MCP session doesn't translate into one outbound GET per tool
call. Refresh failures log at debug only and never propagate to the
user-facing result. 401/403 surface as auth-specific diagnostics
naming `NO_TICKETS_TOKEN`. Cold-path concurrent callers both fetch
(last-writer-wins on identical data — benign, documented).

`crates/nt-mcp/src/fixtures.rs` deleted. The wire output strips
`deprecatedAt` so only the five identity dimensions (id, domain,
entity, action, version) cross the wire; `deprecated_at` lives only
on the internal `EventTypeSpec` as a filter dimension.

Test surface: 13 mcp integration tests (up from 8 fixture-based) +
3 unit tests on `RegistryCache` (throttle within window / past
window / `is_deprecated` direction) + 4 unit tests on
`parse_registry_refresh_interval` + 4 unit tests on
`past_throttle_window`. cargo-mutants on the changed files: 27
mutants, 24 caught, 3 unviable, 0 surviving.

### 24. Extract `nt-core` shared crate
status: completed
commitSha: 0410215

Once Task 19's duplication has a second consumer (Task 20 or Task 21), extract the shared HTTP/auth/urls code into a `crates/nt-core/` workspace member. Both `nt-cli` and `nt-mcp` depend on it. Eliminates the drift risk introduced by Task 19's deliberate duplication.

This sub-task is bookkeeping — no new functionality, just code motion + import updates. Pinned by the existing test suites of both consumers continuing to pass.

**Completed 2026-05-15.** `nt-core` extracted with four modules:
`encoding` (PATH_SEGMENT + helper), `url` (api_url join/trim),
`http` (get_raw / post_json), `error` (Transport / Body /
InvalidJson / HttpStatus). nt-mcp's `publish_event` and
`describe_event_type` now delegate URL composition, percent-
encoding, and HTTP plumbing to nt-core; a small adapter
`nt-mcp::error_map::transport_to_mcp` bridges the generic
`nt_core::Error` to `McpError`. Status-code semantic mapping
(404 / 401 / 403 / non-2xx → tool-specific McpError wording)
stays inline at each tool handler because the wording is per-
resource.

nt-cli stays on its own `transport.rs` for now — its
`TransportError::Network(reqwest::Error)` keeps the typed
reqwest error for `is_timeout()` / `is_connect()` introspection,
which an nt-core migration would lose. Task 25's split of
`transport.rs` is the right time to revisit.

Test surface after refactor:
- nt-core: 25 unit + 1 doctest (encoding, url, http, error)
- nt-mcp: 54/54 unchanged at the integration layer
- cargo-mutants on nt-core: 5/5 caught (3 unviable, 0 surviving)
- Workspace clippy `-D warnings` clean

### 25. File granularity — split files > 500 LOC
status: completed
commitSha: a975650

Diffs across the Rust crates are getting hard to read because several files have grown past 500 LOC during the per-task ratchet of Tasks 15–21. Before we cut a public release in Task 6, split them into smaller modules so per-task diffs stay scoped.

Audit on 2026-05-15 (production + test files only; excludes node_modules / target / dist / coverage):

| File | LOC | Plan |
|---|---|---|
| `crates/nt-mcp/tests/mcp.rs` | 2634 | Split into `tests/mcp/` submodules with a `common.rs` harness (McpClient, helpers) and one file per tool (`list_event_types.rs`, `publish_event.rs`, `describe_event_type.rs`). Each ~500–1500 LOC. |
| `crates/nt-cli/tests/publish.rs` | 1739 | Same submodule pattern, split by feature surface (happy-path, batch, retry, source-detect, auth, output). |
| `crates/nt-cli/src/commands/publish.rs` | 851 | Split by concern into `commands/publish/{mod,envelope,retry,source_resolve,output}.rs`. ~150–200 LOC each. |
| `crates/nt-cli/src/commands/publish_batch.rs` | 846 | Same per-concern split as `publish.rs`. |

Out of scope:
- `crates/nt-cli/src/transport.rs` (767 LOC) — Task 24 extracts most of it into `nt-core`, so this shrinks naturally without a separate split.
- `src/cli.ts` and the TS test files (`src/core/__tests__/parser-mutants.test.ts`, `src/cli/commands/publish/batch.test.ts`, `src/mcp/tools/handlers.test.ts`, `src/__tests__/init-cli-e2e.test.ts`) — all retired with Task 12.

**Why this blocks Task 6:** once we publish a release, contributors arrive and start filing PRs. PRs against 2000-line test files are unreviewable in practice; the file structure ossifies under the weight of inbound changes. Split first, distribute second.

This is bookkeeping — no behaviour change. Pinned by every existing test suite continuing to pass after the moves.

**Files to modify/create:**
- `crates/nt-mcp/tests/mcp.rs` → `crates/nt-mcp/tests/mcp/{common,list_event_types,publish_event,describe_event_type}.rs` + a thin top-level `mcp.rs` re-export
- `crates/nt-cli/tests/publish.rs` → `crates/nt-cli/tests/publish/{...}.rs`
- `crates/nt-cli/src/commands/publish.rs` + `publish_batch.rs` → per-concern submodules

### 6. Distribution pipeline via `cargo-dist`
status: completed
commitSha: da19515
depends_on: [23, 24, 25, 11, 12]

**Why Task 11 (self-update) is a hard prerequisite, not a follow-up:**
the moment we publish v0.1.0 to install.sh / GH Releases, every
direct-download user is pinned to whatever version they downloaded.
Without `nt self-update` in v0.1.0, they have no in-binary path to
v0.1.1 — they have to remember the curl command, re-run it, and
trust that the new install.sh URL hasn't moved. Cutting v0.1.0
without self-update means v0.1.0 users are effectively forked: an
unknown subset stays stuck on the inaugural release and we have no
mechanism to migrate them.

Self-update has to ship in the *first* binary we publish, not the
second. So Task 11 precedes Task 6 in the dependency graph even
though it sounds like an enhancement.

(Package-manager installs — Homebrew, Scoop, cargo, apt/yum — are
exempt: those users update via their manager. Task 11 detects the
install path and redirects appropriately. The blocker only applies
to the install.sh / direct-download cohort, but they're the
biggest cohort by design.)

**Why Task 12 (retire TS CLI + MCP code) is also a hard
prerequisite:** the repo state at the moment of the first public
release is the reference any contributor or curious user lands on.
Shipping v0.1.0 with `src/cli/`, `src/cli.ts`, `src/mcp/`, and
`bin/no-tickets.js` still present advertises a dual TS+Rust surface
that no longer exists in practice — the TS surface is unmaintained,
the Rust binary is canonical. Two concrete failures from that state:

1. **`npx @magic-ingredients/no-tickets` keeps serving the old TS
   CLI** to anyone whose first instinct is npm, directly contradicting
   the Homebrew/Scoop/install.sh release we just announced. No
   backcompat is being kept (`project_no_v1_backcompat` is the explicit
   call), so leaving the npm package on the air is a foot-gun, not a
   migration path.
2. **Contributor PRs land on dead code.** A new contributor who
   reads the README, clones the repo, and modifies what they think is
   the canonical CLI may spend hours fixing `src/cli/*.ts` before
   realising it doesn't ship.

Task 12 retires `src/cli/`, `src/cli.ts`, `src/mcp/`,
`bin/no-tickets.js`, and either strips the npm package to a redirect
placeholder or retires it entirely. It must land before Task 6 so
v0.1.0 ships a clean repo with a single canonical surface.

**Why Task 23 (real-server `list_event_types`) is also a hard
prerequisite:** the Task 2 spike implemented `list_event_types`
against `crates/nt-mcp/src/fixtures.rs` — a hand-rolled list of
example event types baked into the binary at build time. That was
the right shortcut for the spike, but it means the tool as it
stands today returns the binary author's idea of what event types
exist, NOT what the caller's project actually has registered. Two
concrete failure modes if we ship it as-is in v0.1.0:

1. **Wrong answers in production.** A new user installs the binary,
   calls `list_event_types`, sees `billing.invoice.issued.v1` /
   `ai.task.completed.v1` etc. — the fixture set — and tries to
   `publish_event` against them. Their project doesn't have those
   registered, so the server rejects. The tool's documentation says
   "type ids this caller can publish" — fixtures aren't that.
2. **Fixtures drift the moment we publish.** Every server-side
   registry change leaves the binary's fixture list stale, and
   binaries can't be hot-updated. Task 11's self-update helps the
   *binary* stay current; it does nothing for the *data* the binary
   advertises.

Task 23 switches `list_event_types` to a real `GET /v1/registry/
event-types` call (in-memory cache + async refresh, mirroring TS
`RegistryClient`). After that lands, the tool reflects the caller's
project, not the build's fixtures. It MUST land before Task 6 — the
TS reference's whole reason for the tool existing is "tell me what
I can publish in this project," and fixtures don't answer that
question for real users.

Single config block in `Cargo.toml` drives the full distribution surface: cross-compile CI matrix, GitHub Releases workflow, install script (`install.sh`), Homebrew formula publish-and-update, Scoop manifest publish-and-update. Replaces what would otherwise be four separate hand-rolled workflows.

`cargo-zigbuild` is the underlying cross-compile toolchain (configurable through cargo-dist). Five required targets: `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`. Secondary glibc target available as opt-in artifact.

Outputs per release:
- Tarballs / zips per target with sha256 checksums
- Updated Homebrew formula in `magic-ingredients/homebrew-tap`
- Updated Scoop manifest in `magic-ingredients/scoop-bucket`
- `install.sh` published with the release (mirrored by Task 11 hosting)
- Tagged GH Release with all artifacts

Out of `cargo-dist`'s scope (handled in separate tasks): deb/rpm packaging (Task 9), `cargo install` channel (Task 8).

**Files to modify/create:**
- `Cargo.toml` — `[workspace.metadata.dist]` config block, target list, installer set
- `.github/workflows/release.yml` — generated by `cargo dist init`, committed
- `magic-ingredients/homebrew-tap` repo bootstrap (or sub-tree) — receives generated formula on each release
- `magic-ingredients/scoop-bucket` repo bootstrap (or sub-tree) — receives generated manifest on each release

**Acceptance:**
- Tag push produces GH Release with five-target artifacts + checksums
- `brew install magic-ingredients/tap/no-tickets` works on macOS + Linux
- `curl -fsSL https://get.no-tickets.com | sh` produces a working `nt` on Linux/macOS

Scoop is out of cargo-dist 0.31.0's installer set (shell/powershell/npm/homebrew/msi only) and lands as its own task (Task 34).

**Resolution note (2026-05-20):** Pipeline shipped at v0.1.0. Scaffolding commit was da19515 (feat); follow-up fixes 9567792 / 2151e4b (actions: write permissions), 96a26a7 / f42826e / b27d2ed (SCHEMAS_READ_TOKEN injection) landed before the first green release. See Task 29's resolution note for the full smoke-test narrative + the four failure modes that surfaced + their resolutions. All three acceptance criteria verified post-flip-to-public: 5-target archives + sha256 sidecars + both installers in the v0.1.0 release; `Formula/no-tickets.rb` committed to magic-ingredients/homebrew-tap by the publish-homebrew-formula job; `curl get.no-tickets.com` returns 200 text/x-shellscript.

### 7. npm wrapper package (migration path for current users)
status: superseded
commitSha: null

**Superseded — no backcompat required.** The event-repository rewrite explicitly does not preserve the existing `@magic-ingredients/no-tickets` consumer surface (no push v2 / no legacy schema continuity), so a transparent npm-side migration is unnecessary. Existing npm users will install the Rust binary via brew/scoop/cargo/install.sh on next setup; no postinstall shim required.

A TS wrapper for *new* programmatic-from-JS use cases is deferred to **Phase 4** alongside Python + Go wrappers — built on demand, not for migration.

### 8. cargo publish
status: not_started

Publish `nt-cli` and `nt-mcp` to crates.io for the Rust-ecosystem `cargo install` channel.

**Files to modify/create:**
- crate metadata in `Cargo.toml`
- `.github/workflows/publish-crates.yml`

### 9. deb / rpm packaging
status: not_started

Apt and yum repositories for Linux server installs. Hosted on GitHub Pages or a CDN. Out of `cargo-dist` scope — hand-rolled.

**Files to modify/create:**
- `.github/workflows/build-debs.yml`
- `.github/workflows/build-rpms.yml`
- repo manifest files

### 10. get.no-tickets.com hosting + install.sh redirect
status: completed
commitSha: cd5d25b

Picked Cloudflare Worker. Provisioned via the dashboard, then captured as IaC in `infra/get-no-tickets/` (wrangler.toml + src/index.js + README). Worker proxies `github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-installer.sh`, returns it with `Content-Type: text/x-shellscript` and a 5-min edge cache. Custom domain `get.no-tickets.com` bound via `custom_domain = true` in wrangler — DNS + cert auto-managed.

End-to-end verification (`curl -fsSL https://get.no-tickets.com | sh` actually installs working binaries) happens once a tag is pushed and the upstream installer exists — captured in Task 29.

### 11. `nt self-update` subcommand
status: completed
commitSha: aec8657

Add a `nt self-update` command using the `self_update` crate. Scoped specifically to install.sh / direct-download users — package-manager installs (Homebrew, Scoop, cargo, apt/yum) update via their package manager and don't go through this path.

Behavior:
- Reads the latest release from GitHub Releases (same repo as Task 6's artifacts)
- Downloads the target-matched binary
- Verifies sha256 against the published checksum
- Replaces the running binary atomically (`self_update` handles the platform-specific swap dance)
- Prints version-before / version-after for confirmation

Detection: on launch, if the binary detects it was installed via a package manager (e.g., resolved path is under a known manager prefix like `/opt/homebrew`, `/usr/local/Cellar`, `~/.cargo/bin`, `node_modules/.bin`), `nt self-update` prints a message directing the user to their package manager instead of running the swap. Otherwise it proceeds.

**Files to modify/create:**
- `crates/nt-cli/src/commands/self_update.rs`
- `crates/nt-cli/tests/self-update.rs` — smoke test against a staging release (mocked GH Releases endpoint)

**Acceptance:**
- `nt self-update` on an install.sh-installed binary upgrades it to the latest release and verifies sha256
- `nt self-update` on a Homebrew-installed binary prints the package-manager redirect message and exits 0 without modifying anything
- Failure modes (network down, sha256 mismatch, downgrade attempt) map to documented exit codes from Task 26's structured-error contract

### 12. Retire TS CLI + MCP code
status: completed
commitSha: 5eb5d23

Once the Rust binary covers the full surface and self-update is in place (Task 11), delete `src/cli/`, `src/cli.ts`, `src/mcp/`, related tests. The npm package retires entirely from this repo — no backcompat shim. Phase 4 will reintroduce per-language wrapper packages (including TS) as their own publishing surface when adoption justifies it.

**Files to modify/delete:**
- `src/cli/` (delete)
- `src/cli.ts` (delete)
- `src/mcp/` (delete)
- `bin/no-tickets.js` (delete — no replacement shim)
- `package.json` — retire the npm package or strip to a placeholder, depending on the chosen Phase 4 path

### 13. Documentation: install paths
status: completed
commitSha: 3f2c3bd

README + docs covering:
- Each install channel actually shipping in v0.1.0: `brew install magic-ingredients/tap/no-tickets`, `cargo install no-tickets --locked`, `curl -fsSL https://get.no-tickets.com | sh`, PowerShell installer for Windows, direct tarball download per target
- "Why a binary now?" — performance, no-Node CI, multi-channel distribution
- `nt self-update` — when to use it (install.sh / direct-download), when not to (every package-manager channel)
- Note that scoop is a future-channel (Task 34) so Windows users currently use the PowerShell installer

No TS-migration doc — the rewrite explicitly drops npm backcompat per `project_no_v1_backcompat`, and there's no migration path to write up that wouldn't simply repeat "install via one of the new channels."

**Files to modify/create:**
- `README.md`
- `docs/install.md` (new)

### 14. Publish spike — single event to staging end-to-end
status: completed
commitSha: 4844b43

Third toolchain-validation spike, matching the discipline of Tasks 1
and 2. Validates the **last untested major toolchain piece**: HTTPS
client + Bearer auth header injection + JSON request/response + error
mapping. After this lands, end-to-end staging publish from Rust works
and Task 4 (full CLI surface port) becomes mechanical command-by-
command porting against proven plumbing.

Scope: a single subcommand, single event, no batching, no streaming,
no local schema validation, no source merging. The minimum surface
that crosses the wire.

```
nt publish --type <typeId> --data <json> --project <name>
```

Matches the TS reference at `src/transport/events.ts::publish`:
- POST `{apiUrl}/v1/events`
- Body: JSON array with one event object (`{ type, data, source }`)
- Header: `Authorization: Bearer {token}` (token from Task 1's auth resolution)
- Response success: `{ ingested, deduped, ids }` printed to stdout
- Response error: structured error to stderr, non-zero exit

**Toolchain additions verified by this spike:**
- `reqwest` with `default-features = false` + `rustls-tls` + `webpki-roots`
  features. Pure-Rust TLS, no OpenSSL coupling, static-binary friendly.
- Switch `nt-cli` from sync to `#[tokio::main]` (currently sync; reqwest needs async).
- `wiremock` as a dev-dep for fast offline HTTP wire-contract tests.
- Tracing still gated to stderr only (mirrors the MCP gotcha; CLI binary
  isn't an MCP stream, but the discipline keeps `--json` output clean
  for downstream `| jq` consumers).

**Files to modify/create:**
- `crates/nt-cli/Cargo.toml` — add reqwest, tokio runtime, wiremock dev-dep
- `crates/nt-cli/src/main.rs` — switch to `#[tokio::main(flavor = "current_thread")]`
- `crates/nt-cli/src/transport.rs` (new) — HTTPS client + Bearer header injection + POST helper
- `crates/nt-cli/src/commands/mod.rs` (new) — `pub mod publish; pub mod status;`
- `crates/nt-cli/src/commands/publish.rs` (new) — publish command body
- `crates/nt-cli/src/commands/status.rs` — moved from `src/status.rs` for consistency
- `crates/nt-cli/tests/publish.rs` (new) — wiremock-driven wire-contract tests
- `docs/rust-spike-notes.md` — appended Task 14 findings section

**Tests (the wiremock suite, no real network for unit tests):**
- POST body is a JSON array of one object with field order matching TS
  emission (`type`, `data`, `source`, ...).
- `Authorization: Bearer <token>` is present, token sourced from
  NO_TICKETS_TOKEN env (regression-pin: missing/empty token must not
  send the header AND must fail before the request).
- Success response body printed verbatim to stdout, exit 0.
- 401 → exit 1, stderr names auth failure.
- 403 → exit 1, stderr names permission denied.
- 4xx with a structured error body (unknown event type, validation
  failure) → exit 1, stderr surfaces the server message.
- 5xx / connection refused → exit 1, stderr describes transport failure.
- Field order on the wire body pinned by monotonic-byte-position
  assertion (same pattern as nt status and list_event_types tests).

**Acceptance:**
- All wiremock tests pass.
- Manual smoke against staging: with a real `NO_TICKETS_TOKEN`,
  `nt publish --type <real.type.v1> --data '{...}' --project <name>`
  publishes one event and prints `{"ingested":1,"deduped":0,"ids":[...]}`.
  Confirmed by checking the event lands server-side.
- Mutation review (cargo-mutants) clean on the changed files.
- Adversarial review clean on test-quality and impl-quality.

**Explicitly OUT of scope (deferred to later tasks):**
- Multi-event batches → Task 4
- `--stream` mode → Task 27
- Local JSON Schema validation → Task 3 (server validates anyway, so
  unblocked for now)
- Full structured-error-contract polish (the 7-exit-code table in §
  "Public binary contract") → Task 26; this spike uses exit 0/1 only
- Retry/backoff on transient errors → Task 4
- Source auto-detection / merging → Task 4
- Idempotency keys → Task 4
- All other commands (`init`, `project link/list/unlink`, `validate`,
  `connect`, `disconnect`, `token`) → Task 4

After this spike: the three highest-risk toolchain pieces (clap+config
files; rmcp; reqwest+TLS+auth) are all proven. Task 4 becomes a
mechanical port.

### 15. `nt publish` — optional metadata fields
status: completed
commitSha: 9ca9672

Add the optional metadata flags from `runPublishSingle` (TS reference: `src/cli/commands/publish/single.ts`) to the Rust `nt publish`. These were OOS of the Task 14 spike; closing them here:

- `--subject-type` + `--subject-id` → `subject: { type, id }` on the wire (both required if either present)
- `--source-name` → overrides default `source.name = "nt-cli"`
- `--source-attributes KEY=VALUE …` → merged into `source.attributes` (alongside the existing `project`)
- `--parent <eventId>` → `parentEventId`
- `--trace <id>` → `traceId`
- `--dedupe-key <key>` → `dedupeKey` (unlocks client-side idempotency; this is the substantive one)

Wire-shape parity: each field is OMITTED when absent (no JSON `null` / no empty string), matching TS conditional-spread emission. Field order on the wire follows the TS shape: `type, data, subject?, source, parentEventId?, traceId?, dedupeKey?`.

**Files to modify:**
- `crates/nt-cli/src/main.rs` — clap subcommand args for the new flags
- `crates/nt-cli/src/commands/publish.rs` — extend `EventEnvelope` + `Source`; thread flags through `PublishArgs` and `build_envelope`
- `crates/nt-cli/tests/publish.rs` — wire-body assertions per flag combination
- `crates/nt-cli/src/commands/publish.rs` (inline tests) — `build_envelope` field-order pins

**Acceptance:**
- Each flag emits its documented field on the wire; absent flags emit nothing
- `--subject-type` without `--subject-id` (or vice versa) exits 1 with a usage error
- `--source-attributes` accepts `KEY=VALUE` repeated; rejects malformed pairs with a usage error
- Existing publish tests stay green; new wire-shape tests pin every optional field independently

### 16. `nt publish` — batch mode (`--file` / stdin JSONL or JSON array)
status: completed
commitSha: 1006d82

Port the multi-event batch path from `runPublishBatch`. Reads either a JSON array (`[{event}, {event}, …]`) or JSONL (one event object per line) from `--file <path>` or `-` (stdin). Single POST to `/v1/events` with the array of envelopes.

Distinct from Task 27 (`--stream` mode) — batch is "one finite read → one HTTP call → exit"; stream is "long-lived subprocess, JSONL in/out, persistent".

**Files to modify:**
- `crates/nt-cli/src/main.rs` — `--file`, `-` (stdin) flag handling
- `crates/nt-cli/src/commands/publish.rs` — multi-envelope path
- `crates/nt-cli/src/cli/lib/jsonl.rs`-equivalent (Rust) for line-by-line parsing
- `crates/nt-cli/tests/publish.rs` — batch wire-body tests

**Acceptance:**
- JSON-array input produces a single POST with N envelopes in declaration order
- JSONL input (newline-delimited) parsed identically
- Mixed validation: any one bad envelope rolls up to exit 1 with a per-line error count

### 17. `nt publish` — retry/backoff on transient errors
status: completed
commitSha: 3144c6f

Wrap the HTTP call in a bounded retry loop for transient-class failures (connection refused, 5xx, request timeouts). Exponential backoff with jitter; cap at N attempts. Non-transient (4xx, JSON parse errors, etc.) fails immediately.

**Files to modify:**
- `crates/nt-cli/src/transport.rs` — retry policy on top of `HttpClient`
- `crates/nt-cli/tests/publish.rs` — wiremock scenarios for retry + give-up behaviour

**Acceptance:**
- 5xx then 200 → exit 0 with one retry observed in wiremock
- All-5xx (N attempts) → exit 1 with the last-status surfaced
- 4xx never retries; exit 1 immediately

### 18. `nt publish` — source auto-detection / merging
status: completed
commitSha: 79adaf4

**Scope revision (2026-05-14):** the original task description called for CI-runner auto-detection (`GITHUB_ACTIONS`, `GITLAB_CI`, etc.) and a flag-vs-detected merge order. The TS reference (`src/agent-detect.ts` and its test file `src/__tests__/source-detect.test.ts`) **explicitly rejected** CI auto-detection — provenance is caller-driven via `--source-attribute`. The flag-vs-default merge order is already implemented (Task 15: `name: "nt-cli"` default, override-able by `--source-name`; `attributes` BTreeMap seeded with `project`, augmented by `--source-attribute KEY=VALUE`).

What remains from the TS reference: the **opt-in machine-hash attribute** when `NO_TICKETS_INCLUDE_MACHINE=1` is set. The TS SDK includes it in `detectSource()`; the TS CLI does not (a known omission). Including it on the Rust CLI gives every event an audit-trail attribute identifying the producing machine without leaking the raw hostname.

**Machine-hash mechanics (mirrors `src/agent-detect.ts`):**
- Read `NO_TICKETS_INCLUDE_MACHINE`. Only `"1"` enables the attribute; anything else (unset, empty, "0", "true") leaves the attribute absent.
- Hash = `SHA-256("{salt}:{hostname}")`, lowercase hex, truncated to first 16 chars.
- Salt persisted at `~/.notickets/.machine-salt`, 16 random bytes hex-encoded (32 chars), file mode `0o600` on POSIX.
- On first call: atomic create-or-reuse — if the file exists with non-empty contents, read it; otherwise generate, write with `O_EXCL`-equivalent semantics, and on lost-race re-read the winner's salt.
- Empty / whitespace-only existing salt file → regenerate and overwrite.
- Best-effort: any filesystem failure (read-only `$HOME`, missing perms, etc.) silently drops the attribute. Publish must never fail because the machine hash couldn't be computed.
- `HOME` env var read first (testable via env stubbing), `USERPROFILE` as Windows fallback.

**Files to modify:**
- `crates/nt-cli/src/source_detect.rs` (new) — `machine_hash()` helper + salt persistence
- `crates/nt-cli/Cargo.toml` — add `sha2` dep (already a transitive via `nt-schemas`'s build pipeline; promote to direct)
- `crates/nt-cli/src/commands/publish.rs` — call `machine_hash()` inside `build_metadata`; inject into `attributes` when present
- inline tests for the env-var gate, hash format, salt persistence, race handling, FS-failure tolerance
- `crates/nt-cli/tests/publish.rs` — wire-shape test asserting `attributes.machine` presence/absence under the env var

**Acceptance:**
- With `NO_TICKETS_INCLUDE_MACHINE` unset, `source.attributes.machine` is absent from the wire body (regression-pin: current default behaviour preserved).
- With `NO_TICKETS_INCLUDE_MACHINE=1`, `source.attributes.machine` is a 16-char lowercase hex string.
- The hash is stable across invocations with the same salt + hostname; different salt produces a different hash.
- Salt file lives at `~/.notickets/.machine-salt` with mode `0o600` on POSIX.
- Empty / corrupted salt file is regenerated; FS errors during hash computation result in the attribute being silently omitted (exit code unchanged).
- A `--source-attribute machine=manual-override` flag wins over the auto-computed value (last-wins on BTreeMap insert; pinned by test).

## Acceptance Criteria

- [ ] Rust binary built for all 5 targets via cargo + cross/zigbuild
- [ ] Feature-equivalence smoke matrix passes (CLI commands match TS outputs)
- [ ] MCP server passes Anthropic spec compliance suite
- [ ] All distribution channels deliver the same binary checksums
- [ ] `npm install -g @magic-ingredients/no-tickets` transparently switches users to the Rust binary
- [ ] `curl -fsSL https://get.no-tickets.com | sh` produces a working `nt` on Linux/macOS
- [ ] `brew install magic-ingredients/tap/nt` works on macOS + Linux
- [ ] `scoop install nt` works on Windows
- [ ] `cargo install nt-cli` works from crates.io
- [ ] TS CLI + MCP source removed; TS package contains SDK only

## Repo layout

**Decision (2026-05-11): Cargo workspace lives alongside the existing pnpm package in this repo.** No new repo, no Turborepo / pnpm-workspaces / Nx adoption.

```
no-tickets/                       (existing repo)
├── Cargo.toml                    (NEW — workspace root)
├── crates/                       (NEW)
│   ├── nt-cli/
│   └── nt-mcp/
├── package.json                  (existing TS package — unchanged)
├── src/                          (existing TS source — retired in Task 12)
├── wrappers/                     (existing — reserved for Phase 4 per-language wrappers; not used in this fix)
└── ...
```

`cargo` and `pnpm` ignore each other's files; the two toolchains coexist without a workspace orchestrator. Release tags namespace by language (`nt-v0.1.0` for the Rust binary via `cargo-dist`, `v2.x.y` for the npm package — already in use).

**Why not a separate `no-tickets-rust` repo?**
- Tasks 1–5 are spikes — same-repo iteration is materially faster than cross-repo PRs
- Feature-equivalence smoke matrix (Task 4) needs TS + Rust running side-by-side in one CI job
- After Task 12 retires the TS source, this repo naturally becomes the Rust repo (Phase 4 per-language wrappers may live here or split out then) — no migration required

**Why not introduce Turborepo / pnpm-workspaces now?**
- Adds tooling for zero current benefit — the TS package is one unit, Rust is a separate `cargo` workspace, no shared build graph
- Revisit at Phase 4 when per-language TS/Python/Go wrappers may justify a workspace tool

**Future split is cheap.** Self-contained `crates/` means `git filter-repo --subdirectory-filter crates` produces a clean `no-tickets-rust` repo with full history if/when desired. The decision is reversible.

## Dependencies & Coordination

- **Phase 1 must ship first.** This fix ports Phase 1's surface to Rust; if Phase 1 changes the publish/MCP shape after the Rust port begins, the port has to keep up.
- **Spike dependency order.** Tasks 1, 2, and 2c are independent toolchain spikes covering the three highest-risk integrations (clap+config files; rmcp; reqwest+TLS+auth). Run in any order; all must land before Task 4. Task 4 depends on all three; Task 3 (JSON Schema bundle) is independent and can run in parallel with the spikes.
- **Server-side coordination.** The JSON Schema build artifact must be published as a release asset of `no-tickets-service`. Coordinate with the server team to set up that pipeline (it's a thin transformation step over the existing Zod schemas). Task 14 is **not** blocked by this — server-side validation runs regardless, so staging publish from Rust works without a local-validation pass.
- **Code-signing / notarization** for macOS and Windows binaries — important for end-user trust but deferred to a post-Phase-3 follow-up. Initial release ships unsigned with a documented "right-click to open" workaround on macOS.
- **Auto-update mechanism** (`nt self-update`) — **in scope**, see Task 11. Targets install.sh / direct-download users only (every package-manager channel updates via its own manager). Built on the `self_update` crate. Velopack was evaluated and rejected as GUI-app-shaped and the wrong fit for a CLI/MCP binary.

## Lessons / Open Questions

- ~~**Is `rmcp` ready?**~~ **Resolved 2026-05.** Official SDK, 4.7M+ downloads on crates.io. `#[tool]` macro + stdio transport. Task 2 still runs to confirm the round-trip but no longer carries strategic risk.
- **Perry (TypeScript → native) evaluated and rejected (2026-05-11).** Perry compiles TS to native binaries (~2–5 MB, ~1 ms startup) and would let us reuse much of the existing TS code. Rejected because: (a) generational GC vs Rust's zero-GC matters for long-running MCP latency, (b) no MCP-protocol library equivalent to `rmcp` — would need V8-runtime fallback (15–20 MB binary, partial Node runtime — i.e., back to the Bun-compile rejection criteria), (c) v0.5.x single-vendor project — bus-factor risk for infra at this layer. Re-evaluate if Perry hits 1.0 with an MCP crate before Phase 4.
- **Cargo as a distribution channel** — `cargo install` is heavy (compiles from source for ~minutes). Useful for Rust-ecosystem users but not the primary path. Don't optimize the binary for this.
- **Long-term TS code base** — after Phase 4, what's left in this repo? The thin TS SDK + per-language wrappers. **Repo layout decided for Phases 2/3 (see "Repo layout" above): single repo, Cargo workspace alongside pnpm package.** Revisit whether to split into `no-tickets-rust` vs `no-tickets-ts` once Phase 4 lands and the wrapper count justifies workspace tooling.
