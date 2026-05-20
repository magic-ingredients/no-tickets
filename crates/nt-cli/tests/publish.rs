//! `nt publish` integration tests via wiremock.
//!
//! Mirrors `src/transport/events.ts::publish` and `src/transport/
//! client.ts::request`: POST `/v1/events` with Bearer auth, single-
//! element JSON array body, `{ ingested, deduped, ids }` response.
//!
//! Submodules (one per feature surface, kept under 500 LOC each):
//! - `common`: shared harness (subprocess spawner, wiremock helpers,
//!   wire-body capture, batch-file builder)
//! - `happy_path`: basic POST + wire-shape + field order
//! - `metadata`: Task 15 optional flags + Task 18 machine-hash attribute
//! - `auth`: push-token-from-config.json resolution + env-var escape
//!   hatch + the architectural pin that session credentials never
//!   reach `/v1/events` (`docs/fixes/publish-uses-push-token.md`)
//! - `error_handling`: server error response mapping, network failure,
//!   malformed JSON
//! - `retry`: Task 17 retry/backoff on transient failures
//! - `batch`: Task 16 `--file` / stdin batch mode
//!
//! `#[path]` attributes are required because `tests/publish.rs` is a
//! crate root (each `tests/*.rs` is its own integration test binary),
//! so `mod foo;` would otherwise resolve to `tests/foo.rs` rather
//! than `tests/publish/foo.rs`.

#[path = "publish/common.rs"]
mod common;

#[path = "publish/auth.rs"]
mod auth;
#[path = "publish/batch.rs"]
mod batch;
#[path = "publish/error_handling.rs"]
mod error_handling;
#[path = "publish/happy_path.rs"]
mod happy_path;
#[path = "publish/metadata.rs"]
mod metadata;
#[path = "publish/retry.rs"]
mod retry;
