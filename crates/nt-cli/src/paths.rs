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

use directories::ProjectDirs;

use crate::env::Env;

/// Filename of the session credentials file inside [`config_dir`].
#[allow(dead_code)] // wired by GREEN-phase credentials::load impl (Task 3)
pub const CREDENTIALS_FILE: &str = "credentials";

/// Returns the platform-native config directory used by the CLI.
///
/// The `credentials` file lives directly inside this directory. Task 4
/// of ADR-0002 adds `config.json` as the project/token registry alongside it.
///
/// Resolution order:
/// 1. `NO_TICKETS_HOME=<dir>` (non-empty) → `<dir>/.notickets`
/// 2. Platform-native via `directories` crate
/// 3. `None` if neither resolves
#[allow(dead_code)] // wired by GREEN-phase credentials::load impl (Task 3)
pub fn config_dir(env: &dyn Env) -> Option<PathBuf> {
    if let Some(override_home) = env.var("NO_TICKETS_HOME").filter(|s| !s.is_empty()) {
        return Some(PathBuf::from(override_home).join(".notickets"));
    }
    ProjectDirs::from("com", "magic-ingredients", "no-tickets")
        .map(|d| d.config_dir().to_path_buf())
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
        // Empty override must not short-circuit. We assert against a
        // non-None ProjectDirs result so the test refuses to pass
        // vacuously on a sandbox where ProjectDirs returns None — both
        // sides being None would otherwise let drift through.
        let expected = ProjectDirs::from("com", "magic-ingredients", "no-tickets")
            .map(|d| d.config_dir().to_path_buf())
            .expect("test host must have a resolvable platform config dir");
        let env = HashMapEnv::with(&[("NO_TICKETS_HOME", "")]);
        assert_eq!(config_dir(&env), Some(expected));
    }

    #[test]
    fn config_dir_no_override_returns_platform_native_path() {
        // Pins that paths.rs delegates to ProjectDirs with the
        // ADR-specified qualifier triple ("com", "magic-ingredients",
        // "no-tickets"). Asserts non-None so a sandboxed run where
        // ProjectDirs::from returns None doesn't quietly pass and let
        // qualifier drift through.
        let expected = ProjectDirs::from("com", "magic-ingredients", "no-tickets")
            .map(|d| d.config_dir().to_path_buf())
            .expect("test host must have a resolvable platform config dir");
        let env = HashMapEnv::empty();
        assert_eq!(config_dir(&env), Some(expected));
    }

    #[test]
    fn credentials_file_constant_pins_filename() {
        assert_eq!(CREDENTIALS_FILE, "credentials");
    }
}
