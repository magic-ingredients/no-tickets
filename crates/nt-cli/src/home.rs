//! Home-directory resolution.
//!
//! NO_TICKETS_HOME wins over the OS home dir. On Unix the OS home is HOME;
//! on Windows it's USERPROFILE. Matches the TS reference implementation
//! (`process.env['NO_TICKETS_HOME'] || os.homedir()`).
//!
//! Env-var reads route through the `Env` port so tests can inject a
//! fake without mutating process env.

use std::path::PathBuf;

use crate::env::Env;

/// Platform-conditional name of the OS-home env var. The platform gate
/// lives on the constant; the lookup logic in `home_dir` is platform-
/// agnostic. This keeps the same filter+map expression in a single
/// location, so coverage on either platform exercises both NO_TICKETS_HOME
/// and OS-home branches.
#[cfg(unix)]
const OS_HOME_KEY: &str = "HOME";
#[cfg(windows)]
const OS_HOME_KEY: &str = "USERPROFILE";

pub fn home_dir(env: &dyn Env) -> Option<PathBuf> {
    // Resolves to the first key with a non-empty value. Same filter+map
    // expression for both — the platform-specific bit is the OS-home
    // key name, not the lookup shape.
    env.var("NO_TICKETS_HOME")
        .filter(|s| !s.is_empty())
        .or_else(|| env.var(OS_HOME_KEY).filter(|s| !s.is_empty()))
        .map(PathBuf::from)
}

pub fn credentials_path(env: &dyn Env) -> Option<PathBuf> {
    Some(home_dir(env)?.join(".notickets").join("credentials"))
}

pub fn config_path(env: &dyn Env) -> Option<PathBuf> {
    Some(home_dir(env)?.join(".notickets").join("config.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::HashMapEnv;

    const SENTINEL_HOME: &str = "/sentinel/red-phase/z9q3";

    #[test]
    fn home_dir_returns_injected_no_tickets_home_when_set() {
        let env = HashMapEnv::with(&[("NO_TICKETS_HOME", SENTINEL_HOME)]);
        let resolved = home_dir(&env).expect("resolves");
        assert_eq!(resolved, PathBuf::from(SENTINEL_HOME));
    }

    #[test]
    fn credentials_path_appends_notickets_credentials_to_injected_home() {
        let env = HashMapEnv::with(&[("NO_TICKETS_HOME", SENTINEL_HOME)]);
        let path = credentials_path(&env).expect("resolves");
        assert_eq!(
            path,
            PathBuf::from(SENTINEL_HOME).join(".notickets").join("credentials"),
        );
    }

    #[test]
    fn config_path_appends_notickets_config_to_injected_home() {
        let env = HashMapEnv::with(&[("NO_TICKETS_HOME", SENTINEL_HOME)]);
        let path = config_path(&env).expect("resolves");
        assert_eq!(
            path,
            PathBuf::from(SENTINEL_HOME).join(".notickets").join("config.json"),
        );
    }

    /// Platform-appropriate OS-home value. Tests use this so the same
    /// assertion set runs on both unix and windows.
    #[cfg(unix)]
    const OS_HOME_VALUE: &str = "/from/os/home";
    #[cfg(windows)]
    const OS_HOME_VALUE: &str = "C:\\from\\os\\home";

    #[test]
    fn home_dir_returns_none_when_injected_env_has_no_known_keys() {
        // None of NO_TICKETS_HOME, HOME, USERPROFILE present → unresolvable.
        // This is the branch resolve_urls maps to UrlError::HomeUnresolvable.
        let env = HashMapEnv::empty();
        assert_eq!(home_dir(&env), None);
    }

    #[test]
    fn home_dir_empty_no_tickets_home_falls_through_to_os_home() {
        // Empty NO_TICKETS_HOME must not short-circuit — the OS-home
        // lookup should still resolve.
        let env = HashMapEnv::with(&[
            ("NO_TICKETS_HOME", ""),
            (OS_HOME_KEY, OS_HOME_VALUE),
        ]);
        let resolved = home_dir(&env).expect("falls back to OS home");
        assert_eq!(resolved, PathBuf::from(OS_HOME_VALUE));
    }

    #[test]
    fn home_dir_uses_os_home_key_when_no_tickets_home_absent() {
        let env = HashMapEnv::with(&[(OS_HOME_KEY, OS_HOME_VALUE)]);
        let resolved = home_dir(&env).expect("OS-home key resolves");
        assert_eq!(resolved, PathBuf::from(OS_HOME_VALUE));
    }

    #[test]
    fn home_dir_empty_os_home_key_returns_none() {
        let env = HashMapEnv::with(&[(OS_HOME_KEY, "")]);
        assert_eq!(home_dir(&env), None);
    }
}
