//! Platform-native on-disk paths for the no-tickets CLI.
//!
//! Per ADR-0002:
//! - Default: platform-native via the `directories` crate
//!   (`ProjectDirs::from("com", "magic-ingredients", "no-tickets")`).
//! - Override: `NO_TICKETS_HOME=<dir>` env var. When set (and non-empty),
//!   the binary uses `<dir>/.notickets/` instead.
//!
//! No `HOME`/`USERPROFILE` fallback — the override is the only escape
//! hatch, and platform-native resolution already handles those internally
//! via the `directories` crate.

use std::path::PathBuf;

use crate::env::Env;

/// Returns the directory in which `credentials` and `config.json` live.
///
/// Resolution order:
/// 1. `NO_TICKETS_HOME=<dir>` (non-empty) → `<dir>/.notickets`
/// 2. Platform-native via `directories` crate
/// 3. `None` if neither resolves
pub fn config_dir(_env: &dyn Env) -> Option<PathBuf> {
    // RED phase stub — replaced in GREEN.
    None
}

/// Path to the session credentials file inside [`config_dir`].
pub fn credentials_path(env: &dyn Env) -> Option<PathBuf> {
    Some(config_dir(env)?.join("credentials"))
}

/// Path to the project/token registry file inside [`config_dir`].
pub fn config_path(env: &dyn Env) -> Option<PathBuf> {
    Some(config_dir(env)?.join("config.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::HashMapEnv;
    use directories::ProjectDirs;

    const SENTINEL_HOME: &str = "/sentinel/red-phase/paths-z9q3";

    #[test]
    fn config_dir_uses_no_tickets_home_override_when_set() {
        let env = HashMapEnv::with(&[("NO_TICKETS_HOME", SENTINEL_HOME)]);
        let resolved = config_dir(&env).expect("override resolves");
        assert_eq!(resolved, PathBuf::from(SENTINEL_HOME).join(".notickets"));
    }

    #[test]
    fn config_dir_empty_no_tickets_home_falls_through_to_platform_native() {
        // Empty override must not short-circuit — platform-native should
        // still resolve (or return None, matching ProjectDirs's own answer).
        let env = HashMapEnv::with(&[("NO_TICKETS_HOME", "")]);
        let expected =
            ProjectDirs::from("com", "magic-ingredients", "no-tickets")
                .map(|d| d.config_dir().to_path_buf());
        assert_eq!(config_dir(&env), expected);
    }

    #[test]
    fn config_dir_no_override_returns_platform_native_path() {
        // Test pins that paths.rs delegates to ProjectDirs with the
        // ADR-specified qualifier triple — the test recomputes the same
        // call and compares, so any drift in the triple breaks here.
        let env = HashMapEnv::empty();
        let expected =
            ProjectDirs::from("com", "magic-ingredients", "no-tickets")
                .map(|d| d.config_dir().to_path_buf());
        assert_eq!(config_dir(&env), expected);
    }

    #[test]
    fn credentials_path_joins_credentials_to_config_dir() {
        let env = HashMapEnv::with(&[("NO_TICKETS_HOME", SENTINEL_HOME)]);
        let path = credentials_path(&env).expect("resolves");
        assert_eq!(
            path,
            PathBuf::from(SENTINEL_HOME).join(".notickets").join("credentials"),
        );
    }

    #[test]
    fn config_path_joins_config_json_to_config_dir() {
        let env = HashMapEnv::with(&[("NO_TICKETS_HOME", SENTINEL_HOME)]);
        let path = config_path(&env).expect("resolves");
        assert_eq!(
            path,
            PathBuf::from(SENTINEL_HOME).join(".notickets").join("config.json"),
        );
    }
}
