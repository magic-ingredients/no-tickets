//! `nt token remove <project>` — drop a project entry from the local registry.
//!
//! Local-only. Server-side tokens (if any) are NOT revoked — that's the
//! web UI's responsibility per ADR-0002.

use crate::config;
use crate::env::Env;

pub fn run(env: &dyn Env, project: &str) -> i32 {
    let mut cfg = match config::read(env) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    if cfg.projects.remove(project).is_none() {
        eprintln!(
            "No token registered for project `{project}`. Run `no-tickets token list` to see registered projects.",
        );
        return 1;
    }
    if let Err(e) = config::write(env, &cfg) {
        eprintln!("{e}");
        return 1;
    }
    println!("Removed token for project `{project}`.");
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::token_add;
    use crate::env::HashMapEnv;

    fn env_with_home(home: &std::path::Path) -> HashMapEnv {
        HashMapEnv::with(&[("NO_TICKETS_HOME", home.to_str().unwrap())])
    }

    #[test]
    fn token_remove_drops_existing_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        assert_eq!(
            token_add::run(&env, "demo", "nt_push_abcdefgh", None, false),
            0
        );
        assert_eq!(run(&env, "demo"), 0);
        let cfg = config::read(&env).expect("read");
        assert!(!cfg.projects.contains_key("demo"));
    }

    #[test]
    fn token_remove_errors_when_project_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        let code = run(&env, "ghost");
        assert_eq!(code, 1);
    }

    #[test]
    fn token_remove_preserves_other_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        token_add::run(&env, "alpha", "nt_push_aaaaaaaa", None, false);
        token_add::run(&env, "beta", "nt_push_bbbbbbbb", None, false);
        assert_eq!(run(&env, "alpha"), 0);
        let cfg = config::read(&env).expect("read");
        assert!(!cfg.projects.contains_key("alpha"));
        assert!(cfg.projects.contains_key("beta"));
    }
}
