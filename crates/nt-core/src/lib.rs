//! `nt-core` — shared HTTP / URL / encoding primitives for the
//! `nt-cli` and `nt-mcp` binaries. Stateless and error-generic: each
//! consumer adapts `nt_core::Error` into its own typed error
//! (`TransportError` in nt-cli, `McpError` mapping in each nt-mcp
//! tool) via `From` impls or local match arms.
//!
//! What lives here:
//! - `encoding`: RFC 3986 path-segment percent-encoding (the
//!   `PATH_SEGMENT` ascii-set used to safely interpolate event-type
//!   ids and interaction ids into URL paths).
//! - `url`: API URL composition with trailing-slash normalisation
//!   (`api_url(base, "/v1/events")` → `<base>/v1/events`).
//! - `http`: thin reqwest wrapper providing `get_json` / `post_json`
//!   with Bearer auth. No retry / no backoff / no error mapping —
//!   nt-cli owns those layers above the primitives.
//! - `error`: `Error` enum carrying raw HTTP facts (status + body)
//!   and transport/parse failures. Consumers convert.
//!
//! What deliberately doesn't live here:
//! - Retry / backoff policy (consumer-specific cadence; nt-cli has
//!   the Task 17 retry layer, nt-mcp defers retry to the agent).
//! - Status-code → semantic-error mapping (404 reads as
//!   `invalid_params(\"event type X not found\")` in nt-mcp tools but
//!   as a different shape in nt-cli; each consumer maps locally).
//! - Authentication flows (browser-OAuth lives in nt-cli only).

pub mod encoding;
pub mod error;
pub mod http;
pub mod url;

pub use error::Error;
