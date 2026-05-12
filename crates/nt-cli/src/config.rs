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
use std::path::PathBuf;

use crate::env::Env;
#[allow(unused_imports)] // GREEN-phase impl uses these
use crate::paths;
#[allow(unused_imports)] // GREEN-phase impl uses these
use std::fs;

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
#[allow(dead_code)] // variants constructed by GREEN impl
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

#[allow(dead_code)] // GREEN-phase wiring; consumed by Task 5 token commands
pub fn read(_env: &dyn Env) -> Result<Config, ConfigError> {
    // RED stub.
    Err(ConfigError::HomeUnresolvable)
}

#[allow(dead_code)] // GREEN-phase wiring; consumed by Task 5 token commands
pub fn write(_env: &dyn Env, _config: &Config) -> Result<(), ConfigError> {
    // RED stub.
    Err(ConfigError::HomeUnresolvable)
}

#[allow(dead_code)] // GREEN-phase wiring; consumed by Task 5 token list output
pub fn mask_token(_token: &str) -> String {
    // RED stub.
    String::new()
}

#[allow(dead_code)] // GREEN-phase impl uses this
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
    fn mask_token_returns_last_four_with_short_secret_body() {
        // 4-char body — still emits last-4 (whole body) since len >= 4.
        let masked = mask_token("nt_push_abcd");
        assert_eq!(masked, "nt_push_…abcd");
    }

    #[test]
    fn mask_token_redacts_short_or_malformed_token_completely() {
        // < 4-char body or wrong prefix: emit a safe placeholder that
        // exposes nothing. We don't want partial leaks.
        let masked = mask_token("nt_push_xy");
        assert!(
            !masked.contains("xy"),
            "must not leak secret body even partially; got {masked:?}",
        );
        let masked = mask_token("session_or_whatever");
        assert!(
            !masked.contains("session"),
            "must redact non-push-prefixed tokens entirely; got {masked:?}",
        );
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
