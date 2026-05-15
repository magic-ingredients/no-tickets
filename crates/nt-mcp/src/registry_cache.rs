//! Process-lifetime in-memory cache for the event-type registry.
//!
//! `list_event_types` reads from this cache on every invocation. The
//! first call populates the cache synchronously (cold-fetch); subsequent
//! calls serve the cached rows and fire an async refresh to keep the
//! cache fresh for the next read.
//!
//! Refresh failures are intentionally swallowed (logged at debug level
//! only) — the PRD framing is "Reads from the local cache; refresh
//! fires async": a transient registry failure must NOT propagate to
//! the user-facing tool result when the cache already has data.

use std::sync::{Arc, RwLock};

use nt_core::http::get_raw;
use nt_core::url::api_url;
use rmcp::ErrorData as McpError;
use serde::Deserialize;

use crate::config::EnvConfig;
use crate::error_map::transport_to_mcp;

/// One event-type row as it appears in the `eventTypes` array of
/// `GET /v1/registry/event-types`. Optional fields beyond the five
/// identity dimensions are intentionally narrow — the list endpoint
/// omits `schema` / `uiHints` / etc. for body-size reasons, so the
/// only optional we read here is `deprecatedAt` (drives the
/// `deprecated` filter). Detail-shape fields belong on
/// `describe_event_type`'s response, not here.
#[derive(Debug, Clone, Deserialize)]
pub struct EventTypeSpec {
    pub id: String,
    pub domain: String,
    pub entity: String,
    pub action: String,
    pub version: String,
    /// `null` (or absent) means active; a datetime string means
    /// deprecated as of that timestamp. Wire field name is camelCase
    /// per the TS `eventTypeSpecSchema` — server contract.
    #[serde(rename = "deprecatedAt", default)]
    pub deprecated_at: Option<String>,
}

impl EventTypeSpec {
    /// Predicate matching the TS reference's `isDeprecated()` helper:
    /// non-null + non-absent `deprecatedAt` ⇒ deprecated. A regression
    /// that flipped the comparison would surface in the
    /// `list_event_types_filters_by_deprecated_flag` integration test.
    pub fn is_deprecated(&self) -> bool {
        self.deprecated_at.is_some()
    }
}

#[derive(Deserialize)]
struct ListResponse {
    #[serde(rename = "eventTypes")]
    event_types: Vec<EventTypeSpec>,
}

/// Process-lifetime cache shared across tool invocations on the same
/// `NtServer`. `Arc<RwLock<...>>` over an `Option<Vec<_>>`: `None`
/// before the first successful fetch; `Some(rows)` once populated.
/// Refresh writes through the same lock — a failed refresh leaves
/// the existing rows untouched.
#[derive(Clone, Default)]
pub struct RegistryCache {
    inner: Arc<RwLock<Option<Vec<EventTypeSpec>>>>,
}

impl RegistryCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the cached rows. If the cache is empty (cold), fetch
    /// synchronously, populate, return. If the cache is warm, clone
    /// the rows out under the read lock, spawn an async refresh that
    /// the caller does NOT await, then return the cached snapshot.
    ///
    /// Spawning the refresh from inside `list()` (rather than at a
    /// scheduler layer like the TS reference) keeps the cache self-
    /// contained at this MCP-server scope — there is no external
    /// refresh trigger (no `nt event list` running alongside, no
    /// long-lived daemon). The trade-off: the refresh fires opportun-
    /// istically per-call rather than on a timer; in practice MCP
    /// sessions issue list calls frequently enough that this stays
    /// fresh.
    pub async fn list(
        &self,
        config: &EnvConfig,
        http_client: &reqwest::Client,
    ) -> Result<Vec<EventTypeSpec>, McpError> {
        // Take a read snapshot inside its own scope so the lock is
        // released before the await that follows. Holding an RwLock
        // guard across an `.await` is a deadlock risk under tokio's
        // current-thread runtime (no other task can grab the lock
        // while we're suspended).
        let cached_snapshot = self.inner.read().expect("cache lock not poisoned").clone();

        if let Some(rows) = cached_snapshot {
            // Warm cache: spawn an opportunistic refresh and return
            // the snapshot. Errors during the refresh are logged at
            // debug level and never reach the caller — PRD contract.
            let cache = self.clone();
            let config = config.clone();
            let http_client = http_client.clone();
            tokio::spawn(async move {
                match fetch(&config, &http_client).await {
                    Ok(fresh) => {
                        *cache.inner.write().expect("cache lock not poisoned") = Some(fresh);
                    }
                    Err(e) => {
                        // Debug-level only: a transient registry
                        // failure must not surface anywhere the user
                        // can see it.
                        tracing::debug!(error = ?e, "registry refresh failed; cache preserved");
                    }
                }
            });
            return Ok(rows);
        }

        // Cold cache: blocking fetch. Errors propagate so the caller
        // (handler) can surface the diagnostic. Without data the tool
        // has nothing to return — silent empty array would be worse.
        let fresh = fetch(config, http_client).await?;
        *self.inner.write().expect("cache lock not poisoned") = Some(fresh.clone());
        Ok(fresh)
    }
}

/// One-shot GET against the registry list endpoint. Pure I/O; no cache
/// interaction. Split out so the warm-path refresh can call it from a
/// spawned task without touching the lock until the response is in
/// hand.
async fn fetch(
    config: &EnvConfig,
    http_client: &reqwest::Client,
) -> Result<Vec<EventTypeSpec>, McpError> {
    let url = api_url(&config.api_url, "/v1/registry/event-types");
    let response = get_raw(http_client, &url, &config.token)
        .await
        .map_err(transport_to_mcp)?;

    match response.status {
        // 401/403 → auth-specific diagnostic naming the env var to
        // refresh. Same shape as describe_event_type so a client
        // handling one knows what to do with the other.
        401 | 403 => {
            return Err(McpError::internal_error(
                format!(
                    "authentication failed ({}) — check NO_TICKETS_TOKEN; the server rejected the bearer credential",
                    response.status,
                ),
                None,
            ));
        }
        s if !(200..300).contains(&s) => {
            return Err(McpError::internal_error(
                format!("server returned {}: {}", response.status, response.body),
                None,
            ));
        }
        _ => {}
    }

    let parsed: ListResponse = serde_json::from_str(&response.body)
        .map_err(|e| McpError::internal_error(format!("invalid server JSON response: {e}"), None))?;
    Ok(parsed.event_types)
}
