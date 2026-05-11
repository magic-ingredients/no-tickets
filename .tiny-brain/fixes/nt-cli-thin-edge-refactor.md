---
id: nt-cli-thin-edge-refactor
title: "Align nt-cli with thin-edge / stateless-core / DI-friendly principles"
status: not_started
severity: medium
reported: 2026-05-11T00:00:00.000Z
resolved: null
---

# Fix: Align nt-cli with thin-edge / stateless-core / DI-friendly principles

## Issue Summary

**Reported:** 2026-05-11
**Severity:** medium

The Rust rewrite spike (Tasks 1, 2, 14 of `cross-platform-cli-binary`) landed
`nt-cli` with a working but architecturally compromised shape relative to the
project's stated principles:

> thinnest edge possible (anything that is in/out) with stateless well
> structured DRY core that is easy to test with DI.

`nt-schemas` and the `nt-mcp` tool layer follow this cleanly. `nt-cli` does
not. Three concrete gaps make the `commands::*::run` functions un-unit-testable
in-process — every test must shell out to the compiled binary and run wiremock,
which is slow, hard to debug, and produces brittle string-matching assertions
on stderr.

This fix lands the refactor *before* Task 5 (full CLI port) builds more
surface on the current foundation.

## Root Cause Analysis

### Gap 1 — `commands::publish::run` and `commands::status::run` mix I/O with logic

`crates/nt-cli/src/commands/publish.rs:56` (`run`) currently performs all of:

1. env reads (`resolve_urls` → `std::env::var`)
2. file reads (`resolve_auth` → `credentials::load`)
3. JSON parsing of `--data`
4. envelope construction (pure logic — lines 90–100)
5. HTTP request via `reqwest`
6. stdout / stderr printing
7. exit-code mapping

The pure envelope-construction step (declaration order of `type`/`data`/`source`,
the `source.attributes.project` escape hatch, the `nt-cli` / `CARGO_PKG_VERSION`
defaults) is inlined inside the I/O orchestrator. The same applies to
`StatusOutput` construction in `commands/status.rs:44`.

Concretely, this means the wire-body field-order test
(`tests/publish.rs:168 publish_wire_body_field_order_is_type_data_source`)
must spawn the binary against wiremock and grep the captured request body to
assert that `type` precedes `data` precedes `source`. The same assertion
would be a single line in a unit test if `build_envelope` were a pure
function.

### Gap 2 — `transport::Client` is concrete; no `HttpClient` trait

`crates/nt-cli/src/transport.rs:61` hard-binds `Client` to `reqwest::Client`.
There is no trait abstracting "POST JSON, get JSON or transport error", so
`commands::publish::run` cannot accept a fake. Every publish test in
`tests/publish.rs` (~530 LOC, 10 tests) is forced to be an end-to-end
subprocess+wiremock test. The integration coverage is real and worth
keeping, but **the absence of a seam means we cannot also have fast
in-process unit tests** for the orchestration logic (error mapping, JSON
parse failure paths, missing-token short-circuit, etc.).

### Gap 3 — Global env reads inside `resolve_auth` / `resolve_urls`

`crates/nt-cli/src/auth.rs:59` and `crates/nt-cli/src/urls.rs:118` reach
into `std::env::var(...)` directly. As a consequence:

- Tests must mutate process-level env (`tests/publish.rs:32-44` —
  `NO_TICKETS_HOME`, `NO_TICKETS_API_URL`, `NO_TICKETS_AUTH_URL`,
  `NO_TICKETS_TOKEN`).
- Tests cannot run resolution-branch coverage in parallel without env
  races (the integration tests sidestep this by spawning subprocesses,
  but unit tests can't).
- The "partial pair" error branch (urls.rs:125), the
  `NO_TICKETS_TOKEN` empty-string-falls-through branch (auth.rs:61),
  and the credentials-file fallback (auth.rs:70) all live behind
  process-env reads that an injected `Env` would expose to ordinary
  unit tests.

### What this fix is NOT

- Not a behaviour change. Wire format, error messages, exit codes,
  stdout/stderr contract — all preserved byte-for-byte. Existing
  integration tests in `tests/publish.rs` and `tests/status.rs` are
  the regression net and must pass unchanged.
- Not an MCP refactor. `nt-mcp/src/tools/list_event_types.rs:34`
  already takes its fixtures via DI; `nt-mcp` is the model. No
  changes there.
- Not a `nt-schemas` refactor. Already a stateless core. No changes.

## Reproduction / Evidence

The architectural debt is visible in three places without running anything:

- `crates/nt-cli/src/commands/publish.rs:90-101` — pure envelope literal
  embedded inside `run()`, alongside `serde_json::from_str` on `--data`
  (line 74), `Client::new` (line 82), and `client.post_json(...).await`
  (line 103). Seven concerns; one function.
- `crates/nt-cli/src/transport.rs:61` — `pub struct Client { inner:
  reqwest::Client, ... }`. No trait, no `Box<dyn>`, no generic over
  the transport.
- `crates/nt-cli/tests/publish.rs:28` (`run_nt_publish`) — every test
  case spawns `cargo_bin("nt")` because there is no in-process entry
  point. The function is 40 lines of process-management boilerplate
  used by 10 tests.

## Fix Approach

Three small, independent refactors. Each preserves all existing integration
tests, each opens a unit-test seam, each can land as its own TDD cycle.

### Approach 1 — Extract pure builders

Pull envelope/status construction out of `run()`:

- `commands/publish.rs` → split into `pub fn build_envelope(args:
  &PublishArgs, parsed_data: &Value) -> Vec<EventEnvelope>` (pure) plus
  the existing `run()` shell that calls it. Unit-test the builder
  directly for field order, `source.name`, `source.sdkVersion`,
  `source.attributes.project`.
- `commands/status.rs` → split into `pub fn build_output(auth:
  &ResolvedAuth, urls: &ResolvedUrls) -> StatusOutput` (pure) plus the
  existing `run()` shell. Unit-test the builder for field order and
  the `source` / `tokenType` strings across all `AuthSource`/`TokenType`
  variants.

### Approach 2 — Introduce `HttpClient` trait

```rust
#[async_trait]
pub trait HttpClient {
    async fn post_json(&self, path: &str, body: &Value)
        -> Result<Value, TransportError>;
}
```

`reqwest::Client` wrapper becomes `ReqwestHttpClient: HttpClient`. The
`commands::publish::run` signature changes to accept `&dyn HttpClient`
(or generic `<C: HttpClient>`). `main.rs` wires the concrete
`ReqwestHttpClient`; unit tests inject a `FakeHttpClient` that records
calls and returns canned responses.

The unit-test seam exposes: missing-token short-circuit, malformed `--data`
short-circuit, 401/403/422/5xx error mapping, and the unknown-fields
passthrough — all without subprocess+wiremock.

### Approach 3 — Parameterise env reads via `Env` abstraction

```rust
pub trait Env {
    fn var(&self, key: &str) -> Option<String>;
}
pub struct SystemEnv;
impl Env for SystemEnv {
    fn var(&self, key: &str) -> Option<String> { std::env::var(key).ok() }
}
```

`resolve_auth(env: &dyn Env)` and `resolve_urls(env: &dyn Env, profile:
Option<&str>)` take the env source as a parameter. Production wiring in
`main.rs` passes `&SystemEnv`; unit tests pass a `HashMapEnv` populated
inline.

(`home::home_dir` and `credentials::load` likewise widen to take `&dyn Env`
for the env-var reads, leaving the filesystem reads as the only remaining
unmocked I/O — that boundary is fine; the OS is the OS.)

## Test Plan

### 🔒 Regression Tests (must pass unchanged)

| File | Cases | Status |
|------|-------|--------|
| `crates/nt-cli/tests/publish.rs` | all 10 subprocess+wiremock cases | ❌ |
| `crates/nt-cli/tests/status.rs` | all existing cases | ❌ |
| `crates/nt-mcp/tests/mcp.rs` | all existing cases | ❌ |
| `crates/nt-schemas/tests/validate.rs` | all existing cases | ❌ |

The whole point of this refactor: byte-for-byte parity on the existing
integration tests. Any failure here means the refactor is wrong.

### 🆕 New Tests

| File | Case | Status |
|------|------|--------|
| `crates/nt-cli/src/commands/publish.rs` (inline `#[cfg(test)]`) | `build_envelope` emits `type`/`data`/`source` in that field order | ❌ |
| `crates/nt-cli/src/commands/publish.rs` (inline `#[cfg(test)]`) | `build_envelope` sets `source.name=nt-cli`, `source.sdkVersion=CARGO_PKG_VERSION` | ❌ |
| `crates/nt-cli/src/commands/publish.rs` (inline `#[cfg(test)]`) | `build_envelope` writes `source.attributes.project` from args | ❌ |
| `crates/nt-cli/src/commands/status.rs` (inline `#[cfg(test)]`) | `build_output` emits `authenticated`/`source`/`tokenType`/`apiUrl`/`authUrl` in that order | ❌ |
| `crates/nt-cli/src/commands/status.rs` (inline `#[cfg(test)]`) | `build_output` maps each `AuthSource` and `TokenType` to the correct string | ❌ |
| `crates/nt-cli/tests/publish_unit.rs` (or inline) | `run()` short-circuits with exit 1 when token missing — no HttpClient call recorded | ❌ |
| `crates/nt-cli/tests/publish_unit.rs` | `run()` exits 1 on malformed `--data` before any HttpClient call | ❌ |
| `crates/nt-cli/tests/publish_unit.rs` | `run()` propagates `HttpStatus { 401, body }` to stderr; exit 1 | ❌ |
| `crates/nt-cli/tests/publish_unit.rs` | `run()` propagates `HttpStatus { 422, body }` with server validation message to stderr | ❌ |
| `crates/nt-cli/tests/publish_unit.rs` | `run()` passes through unknown response fields verbatim on stdout | ❌ |
| `crates/nt-cli/src/auth.rs` (inline `#[cfg(test)]`) | `resolve_auth` reads `NO_TICKETS_TOKEN` from injected `Env` (no process env touched) | ❌ |
| `crates/nt-cli/src/auth.rs` (inline `#[cfg(test)]`) | empty `NO_TICKETS_TOKEN` falls through to credentials-file path | ❌ |
| `crates/nt-cli/src/urls.rs` (inline `#[cfg(test)]`) | `resolve_urls` `PartialPair` branch via injected `Env` | ❌ |
| `crates/nt-cli/src/urls.rs` (inline `#[cfg(test)]`) | `resolve_urls` defaults when neither env var set | ❌ |

## Tasks

### 1. Extract pure `build_envelope` and `build_output` builders
End-to-end task: failing unit tests for the pure builders, extract the
literal blocks out of `run()`, run the existing integration tests to
prove byte-for-byte parity. The builders MUST be pure (no I/O, no env,
no time) and accept resolved inputs by reference.

**Files to modify:**
- `crates/nt-cli/src/commands/publish.rs`
- `crates/nt-cli/src/commands/status.rs`

### 2. Introduce `HttpClient` trait and inject into `commands::publish::run`
End-to-end task: define `HttpClient` trait with `post_json`, wrap
`reqwest::Client` as `ReqwestHttpClient`, change `run()` to accept the
client via parameter (or generic), wire `main.rs` to construct the
concrete impl, add in-process unit tests with a fake client covering the
error-mapping branches the subprocess tests currently cover via wiremock.
Existing `tests/publish.rs` MUST continue to pass unchanged.

**Files to modify:**
- `crates/nt-cli/src/transport.rs`
- `crates/nt-cli/src/commands/publish.rs`
- `crates/nt-cli/src/main.rs`
- `crates/nt-cli/Cargo.toml` (likely `async-trait` dep)
- New: `crates/nt-cli/tests/publish_unit.rs` or inline `#[cfg(test)]`

### 3. Parameterise env reads via `Env` trait in `auth` and `urls`
End-to-end task: define `Env` trait + `SystemEnv` production impl,
widen `resolve_auth` and `resolve_urls` signatures to take `&dyn Env`,
update call sites in `commands::publish::run`, `commands::status::run`,
and `main.rs`. Add unit tests exercising the resolution branches with
a `HashMapEnv` (no process env mutation). Existing integration tests
that set process env MUST continue to pass unchanged.

**Files to modify:**
- `crates/nt-cli/src/auth.rs`
- `crates/nt-cli/src/urls.rs`
- `crates/nt-cli/src/home.rs`
- `crates/nt-cli/src/commands/publish.rs`
- `crates/nt-cli/src/commands/status.rs`
- `crates/nt-cli/src/main.rs`
