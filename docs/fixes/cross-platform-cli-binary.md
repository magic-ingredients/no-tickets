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
| **Error variants → typed exit codes** | `thiserror` (lib) + `anyhow` (app) | Maps directly to the structured-error contract: each variant derives an exit code + serializable stderr JSON shape. Task 4a becomes near-mechanical. |
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

- The `--stream` JSONL protocol body (Task 4b) — ~100 LOC of tokio stdio + serde.
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

Port all commands to Rust per the ADR-0002 surface (the task description here predates ADR-0002 — the canonical surface is `init`, `logout`, `publish`, `validate`, `status`, `token add/list/remove`; `project link/list/unlink`, `connect`, `disconnect` are deleted/folded). Use `clap` with the derive API for subcommand parsing. Match flag parsing, error messages, exit codes, JSON output schemas.

**Slice progress (multi-cycle; this task aggregates several TDD cycles):**
- `init`, `logout`, `status`, `token add/list/remove` — landed via the side-fix `implement-adr-0002-cli-surface` (the ADR reshape was the natural port point for those commands)
- `publish` (single-event, spike-scope) — landed via Task 14
- `validate` — landed at fc8175b (this fix, TDD cycle 1 of Task 4)
- _Pending:_ `publish` optional metadata (`--subject-type/--subject-id`, `--source-name`, `--source-attributes`, `--parent`, `--trace`, `--dedupe-key`)
- _Pending:_ `publish` batch mode (`--file` / stdin)
- _Pending:_ `publish` retry/backoff on transient errors
- _Pending:_ `publish` source auto-detection / merging

**Files to modify/create:**
- `crates/nt-cli/src/commands/`
- `crates/nt-cli/tests/cli-equivalence.rs` — uses `assert_cmd` + `predicates` to run the binary against staging and assert stdout/stderr/exit code per command

**Acceptance:**
- Feature-equivalence smoke matrix (above) passes for every command

### 4a. Structured error contract on stderr + exit codes
status: not_started

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

### 4b. `--stream` mode for warm in-process publishing
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

### 5. Full MCP server surface port
status: in_progress

Port all tools and discovery flow to the Rust MCP server. Match the TS server's tool descriptors, input schemas, response shapes.

**Scope clarification (2026-05-14):** the TS `create-server.ts` wires only `validate` (legacy `.notickets/` directory checker) and `status` into the actually-exposed MCP surface. The richer toolset (`list_event_types`, `describe_event_type`, `publish_event`, `run_interaction`, `create_subject`) lives in `src/mcp/tools/handlers.ts` but was never registered. The fix doc explicitly names list/describe/publish as in-scope, so the Rust port targets the planned-but-unwired TS surface, not the vestigial validate/status pair.

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
status: not_started

GET `/v1/registry/event-types/{id}` and return the JSON Schema plus a synthesised example payload (`example-synth.ts` equivalent in Rust). Smaller than Task 19 — single GET, response transformation. Naturally pairs with Task 23 (cache).

**Files to modify/create:**
- `crates/nt-mcp/src/tools/describe_event_type.rs` (new)
- Example synthesis port (port `src/lib/example-synth.ts` to Rust)

### 21. `run_interaction` MCP tool
status: not_started

Server-call passthrough. POST `/v1/interactions/{id}` with `{ input, subject? }`, return the event list from the response.

### 22. `create_subject` MCP tool
status: not_started

Server-call passthrough. POST `/v1/subjects` with the subject body, return `{ type, id }`.

### 23. Real-server `list_event_types` (replace fixtures)
status: not_started

Switch `list_event_types` from local fixtures (Task 2 spike) to a real `GET /v1/registry/event-types` call with an in-memory cache + async refresh. Matches TS `RegistryClient` behaviour. Closes out the spike.

### 24. Extract `nt-core` shared crate
status: not_started

Once Task 19's duplication has a second consumer (Task 20 or Task 21), extract the shared HTTP/auth/urls code into a `crates/nt-core/` workspace member. Both `nt-cli` and `nt-mcp` depend on it. Eliminates the drift risk introduced by Task 19's deliberate duplication.

This sub-task is bookkeeping — no new functionality, just code motion + import updates. Pinned by the existing test suites of both consumers continuing to pass.

### 6. Distribution pipeline via `cargo-dist`
status: not_started

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
- `brew install magic-ingredients/tap/nt` works on macOS + Linux
- `scoop install nt` works on Windows
- `curl -fsSL <generated-install.sh-url> | sh` produces a working `nt` on Linux/macOS

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
status: not_started

`cargo-dist` generates the `install.sh` content; this task handles the `get.no-tickets.com` DNS + hosting setup so `curl -fsSL https://get.no-tickets.com | sh` resolves to the cargo-dist-generated install script for the latest release.

Subdomain pick follows the existing `*.no-tickets.com` convention (`api.`, `app.`, `api-staging.`) and the industry pattern (`get.k8s.io`, `get.pnpm.io`, `get.k3s.io`). Avoids the cost + management of a separate vanity domain (`nt.sh` was an earlier proposal).

**Files to modify/create:**
- DNS: add `get` CNAME / A record to the `no-tickets.com` zone pointing at the chosen hosting target
- Hosting target options (pick one):
  - **Cloudflare Worker** that serves the installer.sh content inline (cheapest, edge-cached, sub-50 ms)
  - **GitHub Pages** on a docs/landing repo with a `/install` page and a 200-response root that serves installer.sh
  - **S3 + CloudFront** static-hosting the installer.sh
- Redirect / response rule so a bare `curl -fsSL https://get.no-tickets.com` (no path) returns the installer script with `Content-Type: text/x-shellscript`
- Caching headers: short TTL (≤5 min) so a new release's installer ships quickly

### 11. `nt self-update` subcommand
status: not_started

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
- Failure modes (network down, sha256 mismatch, downgrade attempt) map to documented exit codes from Task 4a's structured-error contract

### 12. Retire TS CLI + MCP code
status: not_started

Once the Rust binary covers the full surface and self-update is in place (Task 11), delete `src/cli/`, `src/cli.ts`, `src/mcp/`, related tests. The npm package retires entirely from this repo — no backcompat shim. Phase 4 will reintroduce per-language wrapper packages (including TS) as their own publishing surface when adoption justifies it.

**Files to modify/delete:**
- `src/cli/` (delete)
- `src/cli.ts` (delete)
- `src/mcp/` (delete)
- `bin/no-tickets.js` (delete — no replacement shim)
- `package.json` — retire the npm package or strip to a placeholder, depending on the chosen Phase 4 path

### 13. Documentation: install paths + migration note
status: not_started

README + docs covering:
- Each install channel (`brew install`, `scoop install`, `cargo install`, `curl install.sh`)
- "Why a binary now?" — performance, no-Node CI, multi-channel distribution
- **Migration note for existing npm users** — the rewrite drops backcompat; install via one of the new channels. Existing `npx no-tickets ...` workflows stop working and must be replaced with `nt ...`. (Phase 4 will reintroduce a TS programmatic wrapper for in-code use; CLI users move to the binary directly.)
- `nt self-update` — when to use it (install.sh / direct-download), when not to (every package-manager channel)

**Files to modify/create:**
- `README.md`
- `docs/install.md` (new)
- `docs/migration-from-ts-cli.md` (new)

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
- `--stream` mode → Task 4b
- Local JSON Schema validation → Task 3 (server validates anyway, so
  unblocked for now)
- Full structured-error-contract polish (the 7-exit-code table in §
  "Public binary contract") → Task 4a; this spike uses exit 0/1 only
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

Distinct from Task 4b (`--stream` mode) — batch is "one finite read → one HTTP call → exit"; stream is "long-lived subprocess, JSONL in/out, persistent".

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
