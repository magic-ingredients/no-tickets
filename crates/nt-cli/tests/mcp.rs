//! Integration tests for the nt-mcp server.
//!
//! Tests spawn the binary as a subprocess, drive the JSON-RPC handshake
//! over stdio, and assert on response shapes + stdout purity (no log
//! lines mixed with protocol frames — see fix doc Task 2 critical note).
//!
//! Hand-rolled minimal MCP handshake rather than using rmcp's client
//! side, so the raw stdout-purity property is directly inspectable.
//!
//! Submodules (one per tool, kept under 1500 LOC each):
//! - `common`: McpClient harness + shared response helpers
//!   (`extract_tool_result_payload`, `collect_error_text`)
//! - `list_event_types`: discovery, shape, filters + cross-cutting
//!   stdout-purity / stderr-logging invariants
//! - `publish_event`: Task 19 — wire shape, dedupe truth table,
//!   identity-spoof protection
//! - `describe_event_type`: Task 20 — schema/example synthesis,
//!   optional-field passthrough, URL encoding
//!
//! `#[path]` attributes are required because `tests/mcp.rs` is the
//! crate root for this integration-test binary; without them, `mod foo;`
//! resolves to `tests/foo.rs` (where the test runner would treat it as
//! a separate binary).

#[path = "mcp/common.rs"]
mod common;

#[path = "mcp/describe_event_type.rs"]
mod describe_event_type;
#[path = "mcp/list_event_types.rs"]
mod list_event_types;
#[path = "mcp/publish_event.rs"]
mod publish_event;
