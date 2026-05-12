//! Session credentials loader. Reads the `credentials` file inside
//! `paths::config_dir` (platform-native by default, `<dir>/.notickets/` when
//! `NO_TICKETS_HOME=<dir>` is set). Mirrors `src/sdk/credentials.ts`:
//! `{ token, email, expiresAt, host }`, JSON; missing / malformed / shape-invalid
//! / expired / env-mismatched all map to non-Valid outcomes.

use serde::Deserialize;
use std::fs;
use time::format_description::well_known::Iso8601;
use time::OffsetDateTime;

use crate::env::Env;
use crate::paths;

/// Shape of the `credentials` file on disk.
///
/// Invariant: a value of this type produced by [`load`] has been
/// shape-validated (all fields present, all strings) AND its
/// `expires_at` is strictly in the future AND its `host` matches the
/// caller-supplied `current_api_url`. Direct construction via struct
/// literal bypasses all three checks — only call sites that have validated
/// elsewhere should construct one directly.
///
/// The `email` field is unused at runtime; it's part of the on-disk shape
/// contract — serde's `Deserialize` requires it to be present as a string,
/// which gives us shape validation for free against the TS reference's
/// `isStoredCredentials` predicate.
///
/// The `host` field tags which environment (api_url) the session token was
/// issued against. See ADR-0002 — sessions don't carry across envs.
#[derive(Deserialize)]
pub struct StoredCredentials {
    pub token: String,
    /// Part of the shape contract (serde requires it as a String, which gives
    /// shape validation for free against the TS `isStoredCredentials`). Unused
    /// at runtime.
    #[allow(dead_code)]
    pub email: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: String,
    pub host: String,
}

/// Outcome of loading the credentials file under the current env.
///
/// The three non-Valid variants give the caller enough context to emit
/// the right user-visible message: `HostMismatch` triggers the
/// "re-run nt init" warning per ADR-0002; `None` is the catch-all for
/// missing / malformed / expired files.
pub enum LoadOutcome {
    Valid(StoredCredentials),
    HostMismatch { stored_host: String },
    None,
}

pub fn load(env: &dyn Env, current_api_url: &str) -> LoadOutcome {
    let Some(path) = paths::config_dir(env).map(|d| d.join(paths::CREDENTIALS_FILE)) else {
        return LoadOutcome::None;
    };
    let Ok(raw) = fs::read_to_string(&path) else {
        return LoadOutcome::None;
    };
    let Ok(parsed) = serde_json::from_str::<StoredCredentials>(&raw) else {
        return LoadOutcome::None;
    };
    if !is_expires_in_future(&parsed.expires_at) {
        return LoadOutcome::None;
    }
    if parsed.host.is_empty() {
        return LoadOutcome::None;
    }
    if parsed.host != current_api_url {
        return LoadOutcome::HostMismatch {
            stored_host: parsed.host,
        };
    }
    LoadOutcome::Valid(parsed)
}

/// Returns true iff the timestamp parses as ISO 8601 AND is strictly after
/// now. Unparseable timestamps return false — deliberate divergence from
/// TS's NaN-comparison accident (see test
/// `status_credentials_unparseable_expires_at_is_not_authenticated`).
fn is_expires_in_future(timestamp: &str) -> bool {
    let Ok(expires) = OffsetDateTime::parse(timestamp, &Iso8601::DEFAULT) else {
        return false;
    };
    is_strictly_after(expires, OffsetDateTime::now_utc())
}

/// Pure helper for the strict-after comparison. Extracted so the
/// boundary case (a == b) can be tested directly — `is_expires_in_future`
/// uses `now_utc()` as one operand, which makes the boundary
/// impossible to pin from outside. Mutation testing flagged the `>`
/// in `is_expires_in_future` as untested for `>` vs `>=`; this helper
/// kills that mutant.
fn is_strictly_after(a: OffsetDateTime, b: OffsetDateTime) -> bool {
    a > b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::HashMapEnv;
    use std::time::Duration;

    const VALID_FUTURE_EXPIRES: &str = "2099-01-01T00:00:00.000Z";
    const API_URL_PROD: &str = "https://api.no-tickets.com";
    const API_URL_STAGING: &str = "https://api-staging.no-tickets.com";

    fn write_creds(home: &std::path::Path, body: &str) {
        let dir = home.join(".notickets");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("credentials"), body).unwrap();
    }

    fn env_with_home(home: &std::path::Path) -> HashMapEnv {
        HashMapEnv::with(&[("NO_TICKETS_HOME", home.to_str().unwrap())])
    }

    #[test]
    fn load_returns_valid_when_host_field_matches_current_api_url() {
        let tmp = tempfile::tempdir().unwrap();
        write_creds(
            tmp.path(),
            &format!(
                r#"{{"token":"nt_session_x","email":"a@b.com","expiresAt":"{VALID_FUTURE_EXPIRES}","host":"{API_URL_PROD}"}}"#,
            ),
        );
        let outcome = load(&env_with_home(tmp.path()), API_URL_PROD);
        match outcome {
            LoadOutcome::Valid(creds) => {
                assert_eq!(creds.token, "nt_session_x");
                assert_eq!(creds.host, API_URL_PROD);
            }
            _ => panic!("expected Valid; got something else"),
        }
    }

    #[test]
    fn load_returns_host_mismatch_when_stored_host_differs_from_current_api_url() {
        let tmp = tempfile::tempdir().unwrap();
        write_creds(
            tmp.path(),
            &format!(
                r#"{{"token":"nt_session_x","email":"a@b.com","expiresAt":"{VALID_FUTURE_EXPIRES}","host":"{API_URL_STAGING}"}}"#,
            ),
        );
        let outcome = load(&env_with_home(tmp.path()), API_URL_PROD);
        match outcome {
            LoadOutcome::HostMismatch { stored_host } => {
                assert_eq!(stored_host, API_URL_STAGING);
            }
            _ => panic!("expected HostMismatch; got something else"),
        }
    }

    #[test]
    fn load_returns_none_when_host_field_is_empty_string() {
        // Empty `host` is treated as malformed — flowing through to
        // HostMismatch { stored_host: "" } would produce a warning that
        // names no env ("issued for "). Reject upstream so the user gets
        // the regular "Not authenticated" prompt instead.
        let tmp = tempfile::tempdir().unwrap();
        write_creds(
            tmp.path(),
            &format!(
                r#"{{"token":"nt_session_x","email":"a@b.com","expiresAt":"{VALID_FUTURE_EXPIRES}","host":""}}"#,
            ),
        );
        let outcome = load(&env_with_home(tmp.path()), API_URL_PROD);
        assert!(matches!(outcome, LoadOutcome::None));
    }

    #[test]
    fn load_returns_none_when_credentials_file_lacks_host_field() {
        // Legacy file (TS CLI shape) without `host` — load must return None
        // so the caller forces a re-init. No silent acceptance.
        let tmp = tempfile::tempdir().unwrap();
        write_creds(
            tmp.path(),
            &format!(
                r#"{{"token":"nt_session_legacy","email":"a@b.com","expiresAt":"{VALID_FUTURE_EXPIRES}"}}"#,
            ),
        );
        let outcome = load(&env_with_home(tmp.path()), API_URL_PROD);
        assert!(matches!(outcome, LoadOutcome::None));
    }

    #[test]
    fn load_returns_none_when_credentials_file_is_expired_even_if_host_matches() {
        let tmp = tempfile::tempdir().unwrap();
        write_creds(
            tmp.path(),
            &format!(
                r#"{{"token":"nt_session_old","email":"a@b.com","expiresAt":"2000-01-01T00:00:00.000Z","host":"{API_URL_PROD}"}}"#,
            ),
        );
        let outcome = load(&env_with_home(tmp.path()), API_URL_PROD);
        assert!(matches!(outcome, LoadOutcome::None));
    }

    #[test]
    fn load_returns_none_when_credentials_file_is_missing() {
        let tmp = tempfile::tempdir().unwrap();
        // No file written.
        let outcome = load(&env_with_home(tmp.path()), API_URL_PROD);
        assert!(matches!(outcome, LoadOutcome::None));
    }

    #[test]
    fn is_strictly_after_returns_true_when_a_is_later_than_b() {
        let b = OffsetDateTime::now_utc();
        let a = b + Duration::from_secs(60);
        assert!(is_strictly_after(a, b));
    }

    #[test]
    fn is_strictly_after_returns_false_when_a_equals_b() {
        // Pins the strict-greater-than semantics. `>=` would return true
        // here; `>` returns false. Mutation testing surfaced this exact
        // boundary.
        let b = OffsetDateTime::now_utc();
        let a = b;
        assert!(!is_strictly_after(a, b));
    }

    #[test]
    fn is_strictly_after_returns_false_when_a_is_earlier_than_b() {
        let b = OffsetDateTime::now_utc();
        let a = b - Duration::from_secs(60);
        assert!(!is_strictly_after(a, b));
    }
}
