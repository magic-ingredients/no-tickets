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

pub fn home_dir(env: &dyn Env) -> Option<PathBuf> {
    // All three branches share the same "set and non-empty" semantics —
    // expressed identically via `.filter(...).map(...)`. Same shape
    // everywhere keeps readers from wondering whether the NO_TICKETS_HOME
    // path was somehow special.
    if let Some(p) = env
        .var("NO_TICKETS_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
    {
        return Some(p);
    }
    #[cfg(unix)]
    {
        env.var("HOME").filter(|s| !s.is_empty()).map(PathBuf::from)
    }
    #[cfg(windows)]
    {
        env.var("USERPROFILE").filter(|s| !s.is_empty()).map(PathBuf::from)
    }
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
        // branch should still resolve. Use the platform-appropriate
        // OS-home key so the test runs on both unix and windows CI.
        #[cfg(unix)]
        let env = HashMapEnv::with(&[
            ("NO_TICKETS_HOME", ""),
            ("HOME", "/from/os/home"),
        ]);
        #[cfg(windows)]
        let env = HashMapEnv::with(&[
            ("NO_TICKETS_HOME", ""),
            ("USERPROFILE", "C:\\from\\os\\home"),
        ]);
        let resolved = home_dir(&env).expect("falls back to OS home");
        #[cfg(unix)]
        assert_eq!(resolved, PathBuf::from("/from/os/home"));
        #[cfg(windows)]
        assert_eq!(resolved, PathBuf::from("C:\\from\\os\\home"));
    }

    #[cfg(unix)]
    #[test]
    fn home_dir_unix_uses_HOME_when_NO_TICKETS_HOME_absent() {
        let env = HashMapEnv::with(&[("HOME", "/unix/host/home")]);
        let resolved = home_dir(&env).expect("HOME resolves");
        assert_eq!(resolved, PathBuf::from("/unix/host/home"));
    }

    #[cfg(unix)]
    #[test]
    fn home_dir_unix_empty_HOME_returns_none() {
        let env = HashMapEnv::with(&[("HOME", "")]);
        assert_eq!(home_dir(&env), None);
    }

    #[cfg(windows)]
    #[test]
    fn home_dir_windows_uses_USERPROFILE_when_NO_TICKETS_HOME_absent() {
        let env = HashMapEnv::with(&[("USERPROFILE", "C:\\windows\\host\\home")]);
        let resolved = home_dir(&env).expect("USERPROFILE resolves");
        assert_eq!(resolved, PathBuf::from("C:\\windows\\host\\home"));
    }

    #[cfg(windows)]
    #[test]
    fn home_dir_windows_empty_USERPROFILE_returns_none() {
        let env = HashMapEnv::with(&[("USERPROFILE", "")]);
        assert_eq!(home_dir(&env), None);
    }
}
