---
id: cross-platform-cli-binary
type: fix
title: Rust rewrite of CLI + MCP server, distributed as a single binary across all major platforms
phase: development
status: not_started
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
| **In-code TS** (`import { publish }`) | Transitional TS scaffold | **TS wrapper (~50–80 LOC, spawns binary in `--stream` mode)** — same import path; transparent migration via the npm wrapper | TS wrapper (unchanged) |
| **In-code Python** | None (raw HTTP or CLI) | None (raw HTTP or CLI) | Python wrapper (~50–80 LOC, `subprocess.Popen` streaming) + Pydantic schemas package |
| **In-code Go** | None (raw HTTP or CLI) | None (raw HTTP or CLI) | Go wrapper (~50–80 LOC, `exec.Cmd` streaming) + struct schemas package |

The "wrapper" pattern is identical across languages. Only the spawn primitive changes. All three call out to the same Rust binary against the same wire contract.

### Phase dependencies

| Phase | Fix | What lands | Depends on |
|---|---|---|---|
| **1** | `publish-shared-surfaces.md` | TS CLI `publish` wired (transitional scaffold); Zod schemas extracted to `@magic-ingredients/no-tickets-schemas`; project registry; flag shape | — |
| **2 (this fix)** | `cross-platform-cli-binary.md` | Full Rust rewrite of CLI + MCP, validating against the JSON Schema build artifact from `no-tickets-service`. **`--stream` mode** for persistent-subprocess wrappers. **Structured-error contract** on stderr. TS CLI and MCP scaffold retired. | Phase 1 (defines the surface to port) |
| **3 (this fix)** | same | Multi-channel distribution: cargo, Homebrew, Scoop, deb/rpm, npm wrapper, install script. npm wrapper makes existing `import { publish }` keep working — body becomes `execFile('nt', ...)` (or `--stream` variant). | Phase 2 |
| **4** | future fix | Python + Go schemas packages (codegen from Zod source, server-side pipeline) + Python + Go wrapper packages (~50–80 LOC each). | Phase 3 (stable binary + structured-error contract + `--stream` contract); also depends on server-side codegen pipeline |

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
| **Install script** at `https://nt.sh/install` | Quickest curl-pipe install on Linux/macOS | Bash script: detects platform, downloads from GH Releases, verifies sha256 |
| **Homebrew tap** | Mac + Linux developers | `brew install magic-ingredients/tap/nt` |
| **Scoop bucket** | Windows developers | `scoop install nt` |
| **cargo install** | Rust ecosystem users | `cargo install nt-cli` (publishes to crates.io) |
| **deb / rpm** | Linux server installs | apt/yum repos hosted on GitHub Pages or a CDN |
| **npm wrapper** | Existing npm users (no migration cost) | `@magic-ingredients/nt`'s postinstall downloads the platform binary from GH Releases (esbuild pattern). `npx no-tickets ...` keeps working. |

The npm wrapper is **the migration path**: existing `npx no-tickets ...` users transparently get the Rust binary on the next install. No breaking change for current consumers.

## Compatibility audit (must verify before committing to Rust)

Specific items to confirm a Rust rewrite is feasible against current TS surface:

- [ ] **MCP Rust crate** — `rmcp` (or alternative) maturity vs Anthropic spec; coverage of stdio transport; tool-handler ergonomics. Spike a `list_event_types` tool round-trip before committing.
- [ ] **JSON Schema validation** — `jsonschema` crate handles the schema features the Zod source uses (refinements, custom `.refine()` predicates may not survive Zod → JSON Schema conversion cleanly). Validate against a sampling of the existing event types.
- [ ] **OAuth callback flow** — `tokio` + `axum` (or `hyper` directly) for the local HTTP listener; cross-platform browser opener (`opener` crate). Confirm 0.0.0.0 binding and timeout semantics match the TS implementation.
- [ ] **Cross-compile toolchain** — `cross` or `cargo-zigbuild` for arm64, Windows, musl Linux. Confirm CI matrix builds cleanly without per-platform runners (cost driver).
- [ ] **Auth file format** — keep `~/.notickets/credentials` and `~/.notickets/config.json` byte-compatible with the TS implementation so users can switch back if needed during rollout.

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
status: not_started

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
status: not_started

Implement one MCP tool (`list_event_types`) using `rmcp` (or chosen alternative). Spawn it as an MCP server, drive it from a Claude Code MCP client. Confirm wire compatibility.

**Files to modify/create:**
- `crates/nt-mcp/` (new)
- `crates/nt-mcp/src/main.rs`
- `crates/nt-mcp/src/tools/list_event_types.rs`
- `crates/nt-mcp/Cargo.toml`

**Acceptance:**
- An IDE-loaded MCP client can list event types via the Rust MCP server
- Round-trip JSON-RPC traces match the TS server

### 3. JSON Schema bundle integration
status: not_started

Pull JSON Schema build artifact from `no-tickets-service` (per release) and embed via `include_bytes!`. Wire `jsonschema` crate validation. Match the validation behavior in `validateEventLocally` from Phase 1.

**Files to modify/create:**
- `crates/nt-cli/build.rs` — fetch + verify schema bundle by version
- `crates/nt-cli/src/validate.rs`
- `crates/nt-mcp/src/validate.rs`

### 4. Full CLI surface port
status: not_started

Port all commands to Rust: `init`, `status`, `publish`, `project link/list/unlink`, `validate`, `connect`, `disconnect`, `token`, `version`, `help`. Match flag parsing, error messages, exit codes, JSON output schemas.

**Files to modify/create:**
- `crates/nt-cli/src/commands/`

**Acceptance:**
- Feature-equivalence smoke matrix (above) passes for every command

### 4a. Structured error contract on stderr + exit codes
status: not_started

Implement the structured-error contract documented in "Public binary contract" above. Every failure case maps to a typed exit code with a single-line JSON object on stderr. This is the contract per-language wrappers parse; backward compatibility is mandatory.

**Files to modify/create:**
- `crates/nt-cli/src/error.rs` — typed error variants + serialization
- `crates/nt-cli/tests/structured-errors.rs` — exit-code + stderr-shape assertions for each error class
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
status: not_started

Port all tools and discovery flow to the Rust MCP server. Match the TS server's tool descriptors, input schemas, response shapes.

**Files to modify/create:**
- `crates/nt-mcp/src/tools/`

**Acceptance:**
- Real agent driving the Rust MCP server produces the same tool outputs as the TS server for the same inputs

### 6. Cross-compile build pipeline
status: not_started

Cargo workspace + `cross` (or `cargo-zigbuild`) producing static binaries for all five targets. CI matrix builds on every PR; tagged builds publish artifacts.

**Files to modify/create:**
- `.github/workflows/build-rust.yml`
- `Cross.toml` (cross-compile config)

### 7. GitHub Releases pipeline
status: not_started

On tag push: build matrix → tarball/zip per target → checksums → GH Release with all artifacts.

**Files to modify/create:**
- `.github/workflows/release-rust.yml`

### 8. npm wrapper package (migration path for current users)
status: not_started

Replace `@magic-ingredients/no-tickets`'s `bin/no-tickets.js` Node entry with a postinstall script that downloads the Rust binary from GH Releases. The CLI surface stays the same; users notice nothing except that `npx no-tickets ...` is faster.

**Files to modify/create:**
- `wrappers/npm-binary/postinstall.js`
- `wrappers/npm-binary/package.json`
- `bin/no-tickets.js` — thin shim invoking the downloaded binary

### 9. Homebrew tap + Scoop bucket
status: not_started

Set up `magic-ingredients/homebrew-tap` and `magic-ingredients/scoop-bucket` repos (or sub-repos) with formulas/manifests that point at GH Releases. Auto-update on each release.

**Files to modify/create:**
- `.github/workflows/update-homebrew.yml`
- `.github/workflows/update-scoop.yml`

### 10. cargo publish
status: not_started

Publish `nt-cli` and `nt-mcp` to crates.io.

**Files to modify/create:**
- crate metadata in `Cargo.toml`
- `.github/workflows/publish-crates.yml`

### 11. Install script + nt.sh redirect
status: not_started

Bash install script: detects platform, downloads from GH Releases, verifies sha256, drops binary into `~/.local/bin` (or `/usr/local/bin` if root). Hosted at `https://nt.sh/install` (or whatever final URL).

**Files to modify/create:**
- `scripts/install.sh`
- DNS / hosting setup for `nt.sh`

### 12. deb / rpm packaging
status: not_started

Apt and yum repositories for Linux server installs. Hosted on GitHub Pages or a CDN.

**Files to modify/create:**
- `.github/workflows/build-debs.yml`
- `.github/workflows/build-rpms.yml`
- repo manifest files

### 13. Retire TS CLI + MCP code
status: not_started

Once the Rust binary covers the full surface and migrates current npm users transparently, delete `src/cli/`, `src/cli.ts`, `src/mcp/`, related tests. The TS package shrinks to the thin SDK surface (Phase 4 will reshape it further).

**Files to modify/delete:**
- `src/cli/` (delete)
- `src/cli.ts` (delete)
- `src/mcp/` (delete)
- `bin/no-tickets.js` (becomes binary shim — see Task 8)
- `package.json` — adjust `bin` and `files`

### 14. Documentation: install paths + migration note
status: not_started

README + docs covering:
- Each install channel (`brew install`, `scoop install`, `cargo install`, `npm install -g`, `curl install.sh`)
- "Why a binary now?" — performance, no-Node CI, multi-channel distribution
- Migration note for npm users — no action required, transparent
- TS SDK is unaffected — programmatic publishing from JS code works the same

**Files to modify/create:**
- `README.md`
- `docs/install.md` (new)
- `docs/migration-from-ts-cli.md` (new)

## Acceptance Criteria

- [ ] Rust binary built for all 5 targets via cargo + cross/zigbuild
- [ ] Feature-equivalence smoke matrix passes (CLI commands match TS outputs)
- [ ] MCP server passes Anthropic spec compliance suite
- [ ] All distribution channels deliver the same binary checksums
- [ ] `npm install -g @magic-ingredients/no-tickets` transparently switches users to the Rust binary
- [ ] `curl -fsSL https://nt.sh/install | sh` produces a working `nt` on Linux/macOS
- [ ] `brew install magic-ingredients/tap/nt` works on macOS + Linux
- [ ] `scoop install nt` works on Windows
- [ ] `cargo install nt-cli` works from crates.io
- [ ] TS CLI + MCP source removed; TS package contains SDK only

## Dependencies & Coordination

- **Phase 1 must ship first.** This fix ports Phase 1's surface to Rust; if Phase 1 changes the publish/MCP shape after the Rust port begins, the port has to keep up.
- **Server-side coordination.** The JSON Schema build artifact must be published as a release asset of `no-tickets-service`. Coordinate with the server team to set up that pipeline (it's a thin transformation step over the existing Zod schemas).
- **Code-signing / notarization** for macOS and Windows binaries — important for end-user trust but deferred to a post-Phase-3 follow-up. Initial release ships unsigned with a documented "right-click to open" workaround on macOS.
- **Auto-update mechanism** (`nt self-update`) — also deferred. Most users update via their package manager.

## Lessons / Open Questions

- **Is `rmcp` ready?** If the Rust MCP crate is not yet feature-complete, Task 5 may need to drop down to JSON-RPC over stdio and implement the protocol directly. Spike (Task 2) settles this.
- **Cargo as a distribution channel** — `cargo install` is heavy (compiles from source for ~minutes). Useful for Rust-ecosystem users but not the primary path. Don't optimize the binary for this.
- **Long-term TS code base** — after Phase 4, what's left in this repo? The thin TS SDK only. Worth considering a repo split: `no-tickets-rust` (CLI/MCP source) vs `no-tickets-ts` (SDK source). Decide closer to Phase 4.
