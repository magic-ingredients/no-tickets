//! `nt token add <project> <push-token> [--label <text>] [--force]`
//!
//! Local-only registry write — no server call. Validates the push-token
//! prefix, refuses to overwrite an existing entry unless `--force` is set,
//! stamps `addedAt` with the current UTC time.

use std::time::SystemTime;
use time::format_description::well_known::Iso8601;
use time::OffsetDateTime;

use crate::config::{self, ProjectEntry};
use crate::env::Env;

pub fn run(
    env: &dyn Env,
    project: &str,
    push_token: &str,
    label: Option<&str>,
    force: bool,
) -> i32 {
    if !push_token.starts_with("nt_push_") {
        eprintln!("Token must start with `nt_push_`. Paste the value from the no-tickets web UI.");
        return 1;
    }
    if project.is_empty() {
        eprintln!("Project name must be non-empty.");
        return 1;
    }
    let mut cfg = match config::read(env) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    if cfg.projects.contains_key(project) && !force {
        eprintln!(
            "Project `{project}` already has a token registered. Re-run with --force to overwrite.",
        );
        return 1;
    }
    let added_at = now_iso8601();
    cfg.projects.insert(
        project.to_string(),
        ProjectEntry {
            push_token: push_token.to_string(),
            added_at,
            label: label.map(str::to_string),
        },
    );
    if let Err(e) = config::write(env, &cfg) {
        eprintln!("{e}");
        return 1;
    }
    println!("Added token for project `{project}`.");
    0
}

fn now_iso8601() -> String {
    let now = OffsetDateTime::from(SystemTime::now());
    now.format(&Iso8601::DEFAULT)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::HashMapEnv;

    fn env_with_home(home: &std::path::Path) -> HashMapEnv {
        HashMapEnv::with(&[("NO_TICKETS_HOME", home.to_str().unwrap())])
    }

    #[test]
    fn token_add_rejects_non_push_prefixed_token() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        let code = run(&env, "demo", "nt_session_oops", None, false);
        assert_eq!(code, 1, "non-push prefix must be rejected");
        // Nothing written.
        let cfg = config::read(&env).expect("read");
        assert!(cfg.projects.is_empty());
    }

    #[test]
    fn token_add_writes_entry_and_returns_zero_on_success() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        let code = run(&env, "demo", "nt_push_abcdefgh", Some("dev box"), false);
        assert_eq!(code, 0);
        let cfg = config::read(&env).expect("read");
        let entry = cfg.projects.get("demo").expect("project present");
        assert_eq!(entry.push_token, "nt_push_abcdefgh");
        assert_eq!(entry.label.as_deref(), Some("dev box"));
        assert!(!entry.added_at.is_empty(), "addedAt must be stamped");
    }

    #[test]
    fn token_add_refuses_overwrite_without_force() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        assert_eq!(run(&env, "demo", "nt_push_abcdefgh", None, false), 0);
        let code = run(&env, "demo", "nt_push_NEWTOKEN", None, false);
        assert_eq!(code, 1, "second add without --force must fail");
        // Original token preserved.
        let cfg = config::read(&env).expect("read");
        assert_eq!(cfg.projects["demo"].push_token, "nt_push_abcdefgh");
    }

    #[test]
    fn token_add_overwrites_when_force_is_set() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        assert_eq!(run(&env, "demo", "nt_push_abcdefgh", None, false), 0);
        let code = run(&env, "demo", "nt_push_NEWTOKEN", Some("new"), true);
        assert_eq!(code, 0);
        let cfg = config::read(&env).expect("read");
        assert_eq!(cfg.projects["demo"].push_token, "nt_push_NEWTOKEN");
        assert_eq!(cfg.projects["demo"].label.as_deref(), Some("new"));
    }

    #[test]
    fn token_add_rejects_empty_project_name() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        let code = run(&env, "", "nt_push_abcdefgh", None, false);
        assert_eq!(code, 1);
    }
}
