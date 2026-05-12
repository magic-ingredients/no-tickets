//! Flat `config.json` project / push-token registry (ADR-0002).
//!
//! Shape:
//! ```json
//! {
//!   "projects": {
//!     "mystaging": {
//!       "pushToken": "nt_push_a0e7...",
//!       "addedAt": "2026-05-11T20:09:00.000Z",
//!       "label": "personal staging"
//!     }
//!   }
//! }
//! ```
//!
//! Unknown top-level keys are preserved verbatim through read → write so
//! adjacent tools (or older CLI versions writing extras) aren't clobbered.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::env::Env;
use crate::paths;

pub const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ProjectEntry {
    #[serde(rename = "pushToken")]
    pub push_token: String,
    #[serde(rename = "addedAt")]
    pub added_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub projects: BTreeMap<String, ProjectEntry>,
    /// Unknown top-level keys preserved on rewrite.
    #[serde(flatten)]
    pub extras: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug)]
#[allow(dead_code)] // consumed by Task 5 token commands
pub enum ConfigError {
    HomeUnresolvable,
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::HomeUnresolvable => write!(f, "Could not resolve config directory."),
            ConfigError::Io(e) => write!(f, "config.json I/O error: {e}"),
            ConfigError::Json(e) => write!(f, "config.json parse error: {e}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        ConfigError::Io(e)
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(e: serde_json::Error) -> Self {
        ConfigError::Json(e)
    }
}

#[allow(dead_code)] // consumed by Task 5 token commands
pub fn read(env: &dyn Env) -> Result<Config, ConfigError> {
    let path = config_path(env).ok_or(ConfigError::HomeUnresolvable)?;
    if !path.exists() {
        return Ok(Config::default());
    }
    let raw = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&raw)?)
}

#[allow(dead_code)] // consumed by Task 5 token commands
pub fn write(env: &dyn Env, config: &Config) -> Result<(), ConfigError> {
    let path = config_path(env).ok_or(ConfigError::HomeUnresolvable)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // Atomic write: per-process unique tmp + rename. Rename is atomic on
    // POSIX within the same filesystem; readers either see the old file
    // or the new file, never a half-written file. The PID + ns suffix
    // avoids two concurrent writers clobbering each other's tmp.
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = path.with_extension(format!("json.tmp.{pid}.{nanos}"));
    let body = serde_json::to_string_pretty(config)?;
    // Create the tmp with the target permissions FROM THE START so the
    // secret never lands on disk under default umask. On non-Unix
    // platforms `OpenOptions::mode` is unavailable; fall back to plain
    // write (ACL discipline is the OS-level responsibility there).
    let write_result = write_secret_atomic(&tmp, body.as_bytes());
    // On any failure, scrub the tmp leftover so a half-written file
    // containing the plaintext token isn't left on disk indefinitely.
    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = fs::rename(&tmp, &path) {
        let _ = fs::remove_file(&tmp);
        return Err(ConfigError::Io(e));
    }
    Ok(())
}

#[cfg(unix)]
fn write_secret_atomic(path: &std::path::Path, body: &[u8]) -> Result<(), ConfigError> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(body)?;
    Ok(())
}

#[cfg(not(unix))]
fn write_secret_atomic(path: &std::path::Path, body: &[u8]) -> Result<(), ConfigError> {
    fs::write(path, body)?;
    Ok(())
}

/// Returns a display-safe form of a push token: `nt_push_…<last4>` for
/// well-formed tokens whose secret body is at least 8 characters long,
/// `nt_push_…****` placeholder otherwise.
///
/// The 8-character minimum stops "mask" from returning a value that is
/// itself the entire secret body when the input is very short. Iterates
/// via `chars()` so non-ASCII input never panics (push tokens are ASCII
/// in practice, but defensive utilities should not panic on bad input).
#[allow(dead_code)] // consumed by Task 5 token list output
pub fn mask_token(token: &str) -> String {
    const PLACEHOLDER: &str = "nt_push_…****";
    const MIN_BODY_LEN: usize = 8;
    const SUFFIX_LEN: usize = 4;
    let Some(rest) = token.strip_prefix("nt_push_") else {
        return PLACEHOLDER.to_string();
    };
    if rest.chars().count() < MIN_BODY_LEN {
        return PLACEHOLDER.to_string();
    }
    let suffix: String = rest
        .chars()
        .rev()
        .take(SUFFIX_LEN)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("nt_push_…{suffix}")
}

fn config_path(env: &dyn Env) -> Option<PathBuf> {
    paths::config_dir(env).map(|d| d.join(CONFIG_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::HashMapEnv;

    fn env_with_home(home: &std::path::Path) -> HashMapEnv {
        HashMapEnv::with(&[("NO_TICKETS_HOME", home.to_str().unwrap())])
    }

    // ─── mask_token ─────────────────────────────────────────────────────

    #[test]
    fn mask_token_returns_last_four_of_secret_for_well_formed_push_token() {
        // ADR-0002: nt_push_a0e7… → nt_push_…<last4>. Pin both the prefix
        // surviving AND the trailing four characters of the secret body.
        let masked =
            mask_token("nt_push_a0e79856da36a60367c38def8ccac62e85b79d81a46863338b21fe86f29ae0c9");
        assert_eq!(masked, "nt_push_…e0c9");
    }

    #[test]
    fn mask_token_redacts_short_secret_body_below_eight_chars() {
        // Body < 8 chars: emit placeholder, never the whole body even
        // when "last 4" would be ambiguous against the full secret. Pins
        // the 8-char threshold defended in the doc-comment.
        for body in ["abcd", "abcdefg", "x", ""] {
            let token = format!("nt_push_{body}");
            let masked = mask_token(&token);
            assert!(
                !masked.contains(body) || body.is_empty(),
                "must not leak body of short token {token:?}; got {masked:?}",
            );
        }
    }

    #[test]
    fn mask_token_redacts_non_push_prefixed_token_completely() {
        let masked = mask_token("session_or_whatever");
        assert!(
            !masked.contains("session"),
            "must redact non-push-prefixed tokens entirely; got {masked:?}",
        );
    }

    #[test]
    fn mask_token_does_not_panic_on_non_ascii_input() {
        // Non-ASCII chars in the body must NOT trigger byte-slice panics.
        // Push tokens are ASCII in practice; this defends against garbage.
        let _ = mask_token("nt_push_é🦀é🦀é🦀é🦀é🦀");
        let _ = mask_token("nt_push_é");
        let _ = mask_token("ℹ️");
    }

    // ─── read / write round-trip ─────────────────────────────────────────

    #[test]
    fn read_returns_default_config_when_file_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        let cfg = read(&env).expect("missing-file should resolve to default config");
        assert_eq!(cfg.projects.len(), 0);
        assert_eq!(cfg.extras.len(), 0);
    }

    #[test]
    fn write_then_read_round_trips_projects() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        let mut cfg = Config::default();
        cfg.projects.insert(
            "mystaging".to_string(),
            ProjectEntry {
                push_token: "nt_push_aaaaaaaa".to_string(),
                added_at: "2026-05-11T20:09:00.000Z".to_string(),
                label: Some("personal staging".to_string()),
            },
        );
        write(&env, &cfg).expect("write");
        let read_back = read(&env).expect("read");
        assert_eq!(read_back, cfg);
    }

    #[test]
    fn write_preserves_unknown_top_level_keys() {
        // Some other tool writes an extra key into config.json. Our rewrite
        // must not clobber it — pin via a manually-seeded file with an
        // extra `legacy` block, then write through Config and reread.
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        let dir = tmp.path().join(".notickets");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(CONFIG_FILE),
            r#"{"projects":{},"legacy":{"keep":"me"}}"#,
        )
        .unwrap();

        let cfg = read(&env).expect("read");
        assert!(cfg.extras.contains_key("legacy"), "extras present on read");

        write(&env, &cfg).expect("write");
        let re_read = read(&env).expect("re-read");
        assert!(
            re_read.extras.contains_key("legacy"),
            "extras survive write→read",
        );
        assert_eq!(
            re_read.extras["legacy"]["keep"], "me",
            "extra value survives verbatim",
        );
    }

    #[cfg(unix)]
    #[test]
    fn write_sets_owner_only_permissions_on_unix() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        write(&env, &Config::default()).expect("write");

        let path = tmp.path().join(".notickets").join(CONFIG_FILE);
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "config.json must be 0600; got {mode:o}");
    }
}
