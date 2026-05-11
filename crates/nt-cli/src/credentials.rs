//! Session credentials loader. Reads the `credentials` file inside
//! `paths::config_dir` (platform-native by default, `<dir>/.notickets/` when
//! `NO_TICKETS_HOME=<dir>` is set). Mirrors `src/sdk/credentials.ts`:
//! `{ token, email, expiresAt }`, JSON; missing / malformed / shape-invalid
//! / expired all map to `None`.

use serde::Deserialize;
use std::fs;
use time::format_description::well_known::Iso8601;
use time::OffsetDateTime;

use crate::env::Env;
use crate::paths;

/// Shape of the `credentials` file on disk.
///
/// Invariant: a value of this type produced by [`load`] has been
/// shape-validated (all three fields present, all strings) AND its
/// `expires_at` is strictly in the future. Direct construction via struct
/// literal bypasses both checks — only call sites that have validated
/// elsewhere should construct one directly.
///
/// The `email` field is unused at runtime; it's part of the on-disk shape
/// contract — serde's `Deserialize` requires it to be present as a string,
/// which gives us shape validation for free against the TS reference's
/// `isStoredCredentials` predicate.
#[derive(Deserialize)]
pub struct StoredCredentials {
    pub token: String,
    #[allow(dead_code)]
    pub email: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: String,
}

pub fn load(env: &dyn Env) -> Option<StoredCredentials> {
    let path = paths::config_dir(env)?.join(paths::CREDENTIALS_FILE);
    let raw = fs::read_to_string(&path).ok()?;
    let parsed: StoredCredentials = serde_json::from_str(&raw).ok()?;
    if !is_expires_in_future(&parsed.expires_at) {
        return None;
    }
    Some(parsed)
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
    use std::time::Duration;

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
