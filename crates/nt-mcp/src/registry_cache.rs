//! Process-lifetime in-memory cache for the event-type registry.
//!
//! `list_event_types` reads from this cache on every invocation. The
//! first call populates the cache synchronously (cold-fetch); subsequent
//! calls serve the cached rows and fire an async refresh (subject to a
//! throttle window — see `min_refresh_interval`) to keep the cache
//! fresh for the next read.
//!
//! Refresh failures are intentionally swallowed (logged at debug level
//! only) — the PRD framing is "Reads from the local cache; refresh
//! fires async": a transient registry failure must NOT propagate to
//! the user-facing tool result when the cache already has data.
//!
//! Cold-path concurrency: two concurrent first-callers BOTH pass the
//! "cache empty" check and both issue the cold fetch. The outcome is
//! last-writer-wins on identical data — benign, not coalesced. A
//! `tokio::sync::Mutex` coordinator would coalesce them but adds
//! complexity for a path that fires once per process lifetime. The
//! `cold_path_concurrent_callers_converge_on_same_data` unit test
//! pins the benign-race contract.

use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

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
    /// per the server's registry contract.
    #[serde(rename = "deprecatedAt", default)]
    pub deprecated_at: Option<String>,
}

impl EventTypeSpec {
    /// A row is deprecated when the server attached a `deprecatedAt`
    /// timestamp; null/absent means active.
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
/// `NtServer`. The rows are wrapped in `Arc` so a snapshot for the
/// read path is a pointer copy (cheap) rather than a `Vec` clone —
/// matters when an MCP session issues many list_event_types calls.
///
/// `min_refresh_interval` throttles the warm-path opportunistic
/// refresh: a busy MCP session calling `list_event_types` rapidly
/// must NOT translate into one outbound GET per call. Default 5s
/// via `NT_REGISTRY_REFRESH_INTERVAL_MS`; tests can set 0 to
/// observe refresh behaviour deterministically.
#[derive(Clone)]
pub struct RegistryCache {
    inner: Arc<RwLock<Option<Arc<Vec<EventTypeSpec>>>>>,
    last_refresh_at: Arc<Mutex<Option<Instant>>>,
    min_refresh_interval: Duration,
}

impl RegistryCache {
    pub fn new(min_refresh_interval: Duration) -> Self {
        Self {
            inner: Arc::new(RwLock::new(None)),
            last_refresh_at: Arc::new(Mutex::new(None)),
            min_refresh_interval,
        }
    }

    /// Return the cached rows. If the cache is empty (cold), fetch
    /// synchronously, populate, return. If the cache is warm, take a
    /// cheap `Arc` snapshot, spawn an opportunistic async refresh
    /// (subject to the throttle window), then return the snapshot.
    ///
    /// The refresh is spawned from inside `list()` rather than at a
    /// separate scheduler layer because the MCP server has no
    /// external refresh trigger — no long-lived daemon, no companion
    /// CLI invocation to coordinate with.
    pub async fn list(
        &self,
        config: &EnvConfig,
        http_client: &reqwest::Client,
    ) -> Result<Arc<Vec<EventTypeSpec>>, McpError> {
        // Snapshot inside an explicit block so the read guard is
        // dropped before the `.await` that follows. Holding an RwLock
        // guard across an `.await` is a deadlock risk under tokio's
        // current-thread runtime (no other task can grab the lock
        // while we're suspended).
        let cached_snapshot: Option<Arc<Vec<EventTypeSpec>>> = {
            let guard = self
                .inner
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard.clone()
        };

        if let Some(rows) = cached_snapshot {
            // Warm cache: maybe spawn an opportunistic refresh.
            self.maybe_spawn_refresh(config, http_client);
            return Ok(rows);
        }

        // Cold cache: blocking fetch. Errors propagate so the caller
        // (handler) can surface the diagnostic. Without data the tool
        // has nothing to return — silent empty array would be worse.
        let fresh = fetch(config, http_client).await?;
        let arc = Arc::new(fresh);
        self.commit(arc.clone());
        Ok(arc)
    }

    /// Spawn a background refresh iff the throttle window has elapsed
    /// since the last refresh attempt. Returns `true` if a refresh
    /// was spawned (used by the unit-level throttle test). The
    /// check-and-update isn't atomic across the `last_refresh_at`
    /// mutex — safe because the worst case is two concurrent warm
    /// callers both spawning a refresh on the same boundary, which
    /// is the benign cold-path race in miniature.
    fn maybe_spawn_refresh(&self, config: &EnvConfig, http_client: &reqwest::Client) -> bool {
        let now = Instant::now();
        let should_spawn = {
            let mut last = self
                .last_refresh_at
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let elapsed = (*last).map(|prev| now.duration_since(prev));
            if past_throttle_window(elapsed, self.min_refresh_interval) {
                *last = Some(now);
                true
            } else {
                false
            }
        };
        if !should_spawn {
            return false;
        }
        let cache = self.clone();
        let config = config.clone();
        let http_client = http_client.clone();
        tokio::spawn(async move {
            match fetch(&config, &http_client).await {
                Ok(fresh) => cache.commit(Arc::new(fresh)),
                Err(e) => {
                    // Debug-level only: a transient registry failure
                    // must not surface anywhere the user can see it.
                    tracing::debug!(error = ?e, "registry refresh failed; cache preserved");
                }
            }
        });
        true
    }

    fn commit(&self, rows: Arc<Vec<EventTypeSpec>>) {
        let mut guard = self
            .inner
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Some(rows);
    }
}

/// Pure predicate for the throttle check, extracted so the boundary
/// semantics are testable without depending on `Instant::now()`
/// resolution between two sequential calls.
///
/// `elapsed = None` means "no prior refresh recorded" — always past
/// the window. `Some(d)` means "d elapsed since the last refresh"
/// — past the window iff `d >= interval`. The `>=` (rather than `>`)
/// makes the boundary inclusive: exactly-at-interval is treated as
/// past-the-window so the throttle releases on time, not one tick
/// later.
fn past_throttle_window(elapsed: Option<Duration>, interval: Duration) -> bool {
    match elapsed {
        None => true,
        Some(d) => d >= interval,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Throttle invariant: a second call within `min_refresh_interval`
    /// of the first must NOT spawn a refresh. Returns `false` on the
    /// second call so a caller (the warm-path) can skip the work.
    #[tokio::test]
    async fn maybe_spawn_refresh_throttles_within_interval() {
        let cache = RegistryCache::new(Duration::from_secs(60));
        let config = EnvConfig {
            api_url: "http://localhost:1".to_string(),
            token: "x".to_string(),
        };
        let client = reqwest::Client::new();
        let first = cache.maybe_spawn_refresh(&config, &client);
        let second = cache.maybe_spawn_refresh(&config, &client);
        assert!(first, "first call must spawn (no prior refresh recorded)");
        assert!(
            !second,
            "second call within the throttle window must NOT spawn",
        );
    }

    /// Throttle invariant pt2: after the window elapses, another
    /// refresh fires. Use a 0-duration interval so any subsequent
    /// call is past the window.
    #[tokio::test]
    async fn maybe_spawn_refresh_fires_again_after_interval() {
        let cache = RegistryCache::new(Duration::ZERO);
        let config = EnvConfig {
            api_url: "http://localhost:1".to_string(),
            token: "x".to_string(),
        };
        let client = reqwest::Client::new();
        let first = cache.maybe_spawn_refresh(&config, &client);
        let second = cache.maybe_spawn_refresh(&config, &client);
        assert!(first, "first call must spawn");
        assert!(
            second,
            "second call past zero-duration window must also spawn",
        );
    }

    // ─── past_throttle_window: boundary semantics ─────────────────────────

    #[test]
    fn past_throttle_window_when_no_prior_refresh_is_always_past() {
        assert!(past_throttle_window(None, Duration::from_secs(60)));
        assert!(past_throttle_window(None, Duration::ZERO));
    }

    #[test]
    fn past_throttle_window_strictly_inside_window_is_throttled() {
        assert!(!past_throttle_window(
            Some(Duration::from_secs(1)),
            Duration::from_secs(5),
        ));
    }

    #[test]
    fn past_throttle_window_at_exactly_interval_is_past() {
        // Inclusive boundary: exactly-at-window must release the
        // throttle on time, not one tick later. Kills the `< → <=`
        // boundary mutant on the underlying comparison.
        assert!(past_throttle_window(
            Some(Duration::from_secs(5)),
            Duration::from_secs(5),
        ));
    }

    #[test]
    fn past_throttle_window_strictly_past_interval_is_past() {
        assert!(past_throttle_window(
            Some(Duration::from_secs(10)),
            Duration::from_secs(5),
        ));
    }

    /// `is_deprecated` predicate direction. A regression that flipped
    /// `is_some` ↔ `is_none` would pass the integration filter test
    /// in some configurations; pin the direction at the type level so
    /// a unit-level mutation can't slip through.
    #[test]
    fn is_deprecated_is_true_iff_deprecated_at_is_some() {
        let active = EventTypeSpec {
            id: "x.y.z.v1".into(),
            domain: "x".into(),
            entity: "y".into(),
            action: "z".into(),
            version: "v1".into(),
            deprecated_at: None,
        };
        let deprecated = EventTypeSpec {
            deprecated_at: Some("2026-01-01T00:00:00.000Z".into()),
            ..active.clone()
        };
        assert!(!active.is_deprecated());
        assert!(deprecated.is_deprecated());
    }
}
