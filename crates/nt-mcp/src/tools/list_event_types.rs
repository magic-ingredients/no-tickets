//! Body of the `list_event_types` MCP tool.
//!
//! Reads from the in-memory `RegistryCache` (cold-fetches on first
//! call, async-refreshes on subsequent ones), then applies the optional
//! `domain` + `deprecated` filters client-side over the cached rows
//! before mapping to the wire envelope `{ types: [{id, domain, entity,
//! action, version}] }`.
//!
//! The output shape strips `deprecatedAt` — only the five identity
//! dimensions cross the wire (matches the TS handler's `.map(t =>
//! ({ id, domain, entity, action, version }))`). Drift here would
//! leak server-internal timestamps; pinned by the wire-shape test.

use rmcp::{model::*, ErrorData as McpError};
use serde::{Deserialize, Serialize};

use crate::config::EnvConfig;
use crate::registry_cache::{EventTypeSpec, RegistryCache};

/// Exact TS-parity description from src/mcp/tools/list-event-types.ts.
/// Pinned here as a constant so the integration test asserts on
/// byte-for-byte equality rather than a substring match. The literal
/// lives in the `#[tool]` attribute over in `server.rs` (rmcp's macro
/// requires a string literal); this constant is the test-side anchor.
#[allow(dead_code)] // Test-only anchor; the literal lives in the #[tool] attribute.
pub const TS_PARITY_DESCRIPTION: &str = "List event types this caller can publish, optionally filtered by domain. Type ids follow domain.entity.action.vN grammar. Reads from the local cache; refresh fires async.";

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct ListEventTypesArgs {
    /// Filter to a single domain prefix.
    #[serde(default)]
    pub domain: Option<String>,
    /// When true, return ONLY deprecated types; when false, only active.
    #[serde(default)]
    pub deprecated: Option<bool>,
}

/// Wire row shape — the five identity dimensions the TS handler
/// surfaces. Owned `String` rather than borrowed `&str` because the
/// caller already cloned the rows out of the cache (the cache write
/// happens on async refresh; holding borrows here would force the
/// caller to keep the read lock open across an await).
#[derive(Debug, Serialize)]
struct Row {
    id: String,
    domain: String,
    entity: String,
    action: String,
    version: String,
}

impl From<EventTypeSpec> for Row {
    fn from(s: EventTypeSpec) -> Self {
        Self {
            id: s.id,
            domain: s.domain,
            entity: s.entity,
            action: s.action,
            version: s.version,
        }
    }
}

#[derive(Debug, Serialize)]
struct Payload {
    types: Vec<Row>,
}

pub async fn handle(
    args: &ListEventTypesArgs,
    config: &EnvConfig,
    http_client: &reqwest::Client,
    cache: &RegistryCache,
) -> Result<CallToolResult, McpError> {
    tracing::info!(
        domain = args.domain.as_deref(),
        deprecated = args.deprecated,
        "list_event_types called",
    );

    let rows = cache.list(config, http_client).await?;

    let filtered: Vec<Row> = rows
        .into_iter()
        .filter(|t| match &args.domain {
            Some(d) => t.domain == *d,
            None => true,
        })
        .filter(|t| match args.deprecated {
            Some(want) => t.is_deprecated() == want,
            None => true,
        })
        .map(Row::from)
        .collect();

    let payload = Payload { types: filtered };
    let json = serde_json::to_string(&payload).expect("Payload always serialises");
    Ok(CallToolResult::success(vec![Content::text(json)]))
}
