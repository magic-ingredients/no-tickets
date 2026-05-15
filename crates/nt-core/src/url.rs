//! API URL composition with trailing-slash normalisation.
//!
//! Concatenates a base URL (typically `NO_TICKETS_API_URL`) with an
//! API-relative path (`/v1/events`, `/v1/registry/event-types/{id}`)
//! into a single well-formed URL, trimming any trailing slash on the
//! base so single-slash routing holds regardless of how the env var
//! was set.

/// Build `<base>/<path>` with the trailing-slash quirk handled.
///
/// - Strips a trailing `/` from `base` if present.
/// - Strips a leading `/` from `path` if present — caller can pass
///   `"/v1/events"` or `"v1/events"`, both work.
///
/// Examples:
/// ```
/// # use nt_core::url::api_url;
/// assert_eq!(api_url("https://api.example", "/v1/events"), "https://api.example/v1/events");
/// assert_eq!(api_url("https://api.example/", "/v1/events"), "https://api.example/v1/events");
/// assert_eq!(api_url("https://api.example/", "v1/events"), "https://api.example/v1/events");
/// ```
pub fn api_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    format!("{base}/{path}")
}

#[cfg(test)]
mod tests {
    use super::api_url;

    #[test]
    fn joins_with_single_slash() {
        assert_eq!(api_url("https://api.example", "/v1/events"), "https://api.example/v1/events");
    }

    #[test]
    fn strips_trailing_slash_from_base() {
        assert_eq!(api_url("https://api.example/", "/v1/events"), "https://api.example/v1/events");
    }

    #[test]
    fn accepts_path_without_leading_slash() {
        assert_eq!(api_url("https://api.example", "v1/events"), "https://api.example/v1/events");
    }

    #[test]
    fn handles_both_sides_naked() {
        assert_eq!(api_url("https://api.example", "v1"), "https://api.example/v1");
    }

    #[test]
    fn multiple_trailing_slashes_collapse_to_zero() {
        // Defensive: a `base = "...//"` (env var bug) shouldn't
        // produce `host///v1/...`. `trim_end_matches('/')` strips
        // them all.
        assert_eq!(api_url("https://api.example///", "/v1/events"), "https://api.example/v1/events");
    }
}
