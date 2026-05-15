//! Adapter from `nt_core::Error` to `McpError`.
//!
//! `nt-core` is generic in errors — it carries raw HTTP facts and
//! transport-layer failure strings, with no opinion on how a
//! consumer wants to surface them. This module is the local adapter
//! that turns those raw facts into `McpError` variants matching the
//! tools' established diagnostic shapes.
//!
//! Status-code semantic mapping (404 → "not found", 401/403 → auth)
//! stays inline in each tool handler because the wording is tool-
//! specific (event type vs interaction vs subject). Only the
//! transport/body/parse layer — which is uniform across tools —
//! lives here.

use nt_core::Error as CoreError;
use rmcp::ErrorData as McpError;

/// Convert a transport-layer `nt_core::Error` (Transport / Body /
/// InvalidJson) into an `McpError`. Non-2xx HTTP responses are NOT
/// the responsibility of this function — when callers use
/// `nt_core::http::get_raw` / `post_json`, they receive a
/// `RawResponse` for any reachable server and only see `Error` for
/// transport failures (connect refused, body read failed, JSON
/// parse failed). The status-code switch happens inline at each
/// tool handler.
///
/// `Error::HttpStatus` is included here for completeness (callers
/// that use `nt_core::http::get_json` and choose not to inspect the
/// status manually will receive this variant for non-2xx) — it maps
/// to the same generic `internal_error` shape the tool handlers
/// already emit for unhandled status codes.
pub fn transport_to_mcp(err: CoreError) -> McpError {
    // Defer the wording to `nt_core::Error`'s Display impl — single
    // source of truth. The previous version reformatted each variant
    // by hand and drifted: `InvalidJson` adapter said "invalid
    // server JSON response" while the canonical Display said
    // "invalid JSON response" (the integration tests pin "invalid"
    // AND "json" lowercase substrings, so both passed — but the
    // contract drift is exactly the kind of cross-crate divergence
    // this adapter is meant to centralise, not introduce).
    //
    // Status-code semantic mapping (404 / 401 / 403 → tool-specific
    // McpError variants) is NOT here — it stays inline at each tool
    // handler because the wording is per-resource.
    McpError::internal_error(err.to_string(), None)
}
