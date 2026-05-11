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
    if let Some(h) = env.var("NO_TICKETS_HOME") {
        if !h.is_empty() {
            return Some(PathBuf::from(h));
        }
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
}
