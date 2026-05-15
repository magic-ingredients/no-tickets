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
        assert_eq!(
            api_url("https://api.example", "/v1/events"),
            "https://api.example/v1/events"
        );
    }

    #[test]
    fn strips_trailing_slash_from_base() {
        assert_eq!(
            api_url("https://api.example/", "/v1/events"),
            "https://api.example/v1/events"
        );
    }

    #[test]
    fn accepts_path_without_leading_slash() {
        assert_eq!(
            api_url("https://api.example", "v1/events"),
            "https://api.example/v1/events"
        );
    }

    #[test]
    fn handles_both_sides_naked() {
        assert_eq!(
            api_url("https://api.example", "v1"),
            "https://api.example/v1"
        );
    }

    #[test]
    fn multiple_trailing_slashes_collapse_to_zero() {
        // Defensive: a `base = "...//"` (env var bug) shouldn't
        // produce `host///v1/...`. `trim_end_matches('/')` strips
        // them all.
        assert_eq!(
            api_url("https://api.example///", "/v1/events"),
            "https://api.example/v1/events"
        );
    }

    // Adversarial review #6: pin behaviour on degenerate inputs.
    // None of these is a sensible production call, but documenting
    // what `api_url` does with empty / single-slash inputs is the
    // right way to nail the contract — a future refactor that
    // chose to return `None` or panic on empties would break the
    // pin here.

    #[test]
    fn empty_path_produces_trailing_slash() {
        // `base + "/" + ""` → `base/`. Caller mistake (path should
        // be non-empty), but the function is total.
        assert_eq!(api_url("https://api.example", ""), "https://api.example/");
    }

    #[test]
    fn empty_base_produces_path_with_leading_slash() {
        // Defensive: an empty `NO_TICKETS_API_URL` (unset → caller
        // should have rejected earlier, but we don't trust callers)
        // produces just `/v1/x`, which will then fail downstream
        // when reqwest parses the URL. The function stays total
        // rather than panicking.
        assert_eq!(api_url("", "/v1/x"), "/v1/x");
    }

    #[test]
    fn slash_only_base_produces_path_with_single_leading_slash() {
        // Edge of the trim: `base = "/"` trims to empty, then the
        // join produces `/v1/x`.
        assert_eq!(api_url("/", "/v1/x"), "/v1/x");
    }
}
