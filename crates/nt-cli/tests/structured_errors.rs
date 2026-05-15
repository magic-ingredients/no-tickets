//! Structured-error contract tests — Task 26.
//!
//! Asserts that every documented `NtError` variant is observable from
//! the binary's outside, in the documented (exit code, stderr JSON
//! shape) pair. The binary writes JSON to stderr when stderr is a
//! pipe; these tests pipe stderr by construction (via `Stdio::piped()`)
//! so the JSON path is exercised. The human-on-TTY render is
//! verified at unit level inside `crate::error::tests::format_for_*`.
//!
//! Scope (matches the Task 26 migration scope):
//! - `nt publish` (single-event path)
//! - `nt validate`
//!
//! Out of scope (separate cleanup tickets):
//! - `nt init`, `nt logout`, `nt status`, `nt token …`, `nt self-update`
//!
//! Test surface organisation: one submodule per command, with a thin
//! shared `common` harness that spawns the binary against an isolated
//! `NO_TICKETS_HOME` and a wiremock URL (for the publish suite).

#[path = "structured_errors/common.rs"]
mod common;

#[path = "structured_errors/publish.rs"]
mod publish;

#[path = "structured_errors/validate.rs"]
mod validate;
