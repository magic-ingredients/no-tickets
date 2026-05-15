//! Body of the `list_event_types` MCP tool.
//!
//! Reads from the in-memory `RegistryCache` (cold-fetches on first
//! call, async-refreshes on subsequent ones), then applies the optional
//! `domain` + `deprecated` filters client-side over the cached rows
//! before mapping to the wire envelope `{ types: [{id, domain, entity,
//! action, version}] }`.
//!
//! The output shape strips `deprecatedAt` — only the five identity
//! dimensions cross the wire. Drift here would leak server-internal
//! timestamps; pinned by the wire-shape test.

use rmcp::{model::*, ErrorData as McpError};
use serde::{Deserialize, Serialize};

use crate::config::EnvConfig;
use crate::registry_cache::{EventTypeSpec, RegistryCache};

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
pub struct ListEventTypesArgs {
    /// Filter to a single domain prefix.
    #[serde(default)]
    pub domain: Option<String>,
    /// When true, return ONLY deprecated types; when false, only active.
    #[serde(default)]
    pub deprecated: Option<bool>,
}

/// Wire row shape — the five identity dimensions an MCP caller sees.
/// Owned `String` rather than borrowed `&str` because the caller is
/// already serving from an `Arc<Vec<EventTypeSpec>>` snapshot and the
/// row map allocates new strings during the filter/clone step.
#[derive(Debug, Serialize)]
struct Row {
    id: String,
    domain: String,
    entity: String,
    action: String,
    version: String,
}

impl From<&EventTypeSpec> for Row {
    fn from(s: &EventTypeSpec) -> Self {
        Self {
            id: s.id.clone(),
            domain: s.domain.clone(),
            entity: s.entity.clone(),
            action: s.action.clone(),
            version: s.version.clone(),
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
        .iter()
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
