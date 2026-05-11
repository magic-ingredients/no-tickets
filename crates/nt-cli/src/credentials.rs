//! `~/.notickets/credentials` loader. Mirrors `src/sdk/credentials.ts`:
//! `{ token, email, expiresAt }`, JSON; missing / malformed / shape-invalid
//! / expired all map to `None`.

use serde::Deserialize;
use std::fs;
use time::OffsetDateTime;
use time::format_description::well_known::Iso8601;

use crate::env::Env;
use crate::home;

/// Shape of `~/.notickets/credentials` on disk.
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
    let path = home::credentials_path(env)?;
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
    expires > OffsetDateTime::now_utc()
}
