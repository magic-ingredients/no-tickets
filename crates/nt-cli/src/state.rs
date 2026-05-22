//! `state.json` — small CLI state file.
//!
//! Currently carries `firstPublishHintShown` for the unattributed-publish
//! hint mechanic (Task 5). Designed to grow additional CLI-side state
//! without proliferating one-flag files in `<config-dir>`. Unknown keys
//! are preserved through read → write so other tools (or older CLI
//! versions writing extras) aren't clobbered.
//!
//! Shape:
//! ```json
//! { "firstPublishHintShown": true }
//! ```

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::env::Env;
use crate::paths;

pub const STATE_FILE: &str = "state.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// `true` after the no-actor publish has printed the one-time hint.
    /// Serialised only when `true` so the default state file is `{}`,
    /// not `{"firstPublishHintShown":false}`.
    #[serde(
        rename = "firstPublishHintShown",
        default,
        skip_serializing_if = "is_false"
    )]
    pub first_publish_hint_shown: bool,
    /// Unknown top-level keys preserved on rewrite.
    #[serde(flatten)]
    pub extras: serde_json::Map<String, serde_json::Value>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug)]
#[allow(dead_code)] // wired into nt error envelope in Task 5
pub enum StateError {
    HomeUnresolvable,
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateError::HomeUnresolvable => write!(f, "Could not resolve config directory."),
            StateError::Io(e) => write!(f, "state.json I/O error: {e}"),
            StateError::Json(e) => write!(f, "state.json parse error: {e}"),
        }
    }
}

impl std::error::Error for StateError {}

impl From<std::io::Error> for StateError {
    fn from(e: std::io::Error) -> Self {
        StateError::Io(e)
    }
}

impl From<serde_json::Error> for StateError {
    fn from(e: serde_json::Error) -> Self {
        StateError::Json(e)
    }
}

#[allow(dead_code)] // consumed by Task 5 publish resolver
pub fn read(env: &dyn Env) -> Result<Option<State>, StateError> {
    let path = state_path(env).ok_or(StateError::HomeUnresolvable)?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)?;
    Ok(Some(serde_json::from_str(&raw)?))
}

#[allow(dead_code)] // consumed by Task 5 publish resolver
pub fn write(env: &dyn Env, state: &State) -> Result<(), StateError> {
    let path = state_path(env).ok_or(StateError::HomeUnresolvable)?;
    let body = serde_json::to_string_pretty(state)?;
    crate::atomic_write::write_atomic(&path, body.as_bytes())?;
    Ok(())
}

/// Clear the hint marker.
///
/// Per PRD: do NOT create `state.json` just to write `firstPublishHintShown:
/// false`. If the file is missing entirely, no-op. If it exists, set the
/// flag to `false` and write back (preserving any other state).
pub fn clear_hint_marker(env: &dyn Env) -> Result<(), StateError> {
    let Some(mut s) = read(env)? else {
        return Ok(());
    };
    s.first_publish_hint_shown = false;
    write(env, &s)
}

pub fn state_path(env: &dyn Env) -> Option<PathBuf> {
    paths::config_dir(env).map(|d| d.join(STATE_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::HashMapEnv;

    fn env_with_home(home: &std::path::Path) -> HashMapEnv {
        HashMapEnv::with(&[("NO_TICKETS_HOME", home.to_str().unwrap())])
    }

    // ─── read / write round-trip ────────────────────────────────────────────

    #[test]
    fn read_returns_none_when_file_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        assert!(read(&env).unwrap().is_none());
    }

    #[test]
    fn write_then_read_round_trips_hint_flag() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        let s = State {
            first_publish_hint_shown: true,
            extras: serde_json::Map::new(),
        };
        write(&env, &s).expect("write");
        let read_back = read(&env).expect("read").expect("file present");
        assert!(read_back.first_publish_hint_shown);
    }

    #[test]
    fn write_omits_false_flag_from_serialised_json() {
        // `{"firstPublishHintShown":false}` is noise — the default-when-
        // absent semantic means a false flag should serialise as `{}`.
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        write(&env, &State::default()).expect("write");

        let path = state_path(&env).expect("path resolves");
        let raw = std::fs::read_to_string(&path).expect("file exists");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("valid json");
        assert!(
            parsed.get("firstPublishHintShown").is_none(),
            "false flag must be omitted; got {raw}",
        );
    }

    #[test]
    fn write_preserves_unknown_top_level_keys() {
        // Other tools / future CLI versions may write extras into
        // state.json. Read → write must not clobber them.
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        let dir = tmp.path().join(".notickets");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(STATE_FILE),
            r#"{"firstPublishHintShown":true,"experimental":{"k":"v"}}"#,
        )
        .unwrap();

        let s = read(&env).expect("read").expect("file present");
        assert!(s.extras.contains_key("experimental"));
        write(&env, &s).expect("re-write");
        let re_read = read(&env).expect("read").expect("file present");
        assert!(re_read.extras.contains_key("experimental"));
        assert_eq!(re_read.extras["experimental"]["k"], "v");
    }

    // ─── clear_hint_marker ──────────────────────────────────────────────────

    #[test]
    fn clear_hint_marker_is_noop_when_file_absent() {
        // PRD: do NOT create state.json just to write false. If the
        // file is absent, clearing is a no-op — the file stays absent.
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        clear_hint_marker(&env).expect("clear on absent file");
        let path = state_path(&env).expect("path resolves");
        assert!(!path.exists(), "must not create state.json; path={path:?}");
    }

    #[test]
    fn clear_hint_marker_resets_flag_when_file_present() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        write(
            &env,
            &State {
                first_publish_hint_shown: true,
                extras: serde_json::Map::new(),
            },
        )
        .expect("seed");

        clear_hint_marker(&env).expect("clear");
        let read_back = read(&env).expect("read").expect("file still present");
        assert!(
            !read_back.first_publish_hint_shown,
            "flag must be cleared after clear_hint_marker",
        );
    }

    #[test]
    fn clear_hint_marker_preserves_other_state() {
        // Clearing the hint must not destroy unrelated extras the file
        // might be carrying.
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        let dir = tmp.path().join(".notickets");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(STATE_FILE),
            r#"{"firstPublishHintShown":true,"experimental":{"k":"v"}}"#,
        )
        .unwrap();

        clear_hint_marker(&env).expect("clear");
        let read_back = read(&env).expect("read").expect("file still present");
        assert!(!read_back.first_publish_hint_shown);
        assert_eq!(read_back.extras["experimental"]["k"], "v");
    }

    #[test]
    fn write_does_not_leave_tmp_files_around() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        write(
            &env,
            &State {
                first_publish_hint_shown: true,
                extras: serde_json::Map::new(),
            },
        )
        .expect("write");
        let dir = tmp.path().join(".notickets");
        for entry in std::fs::read_dir(&dir).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            assert!(
                !name.contains(".tmp."),
                "tmp leftover in config dir: {name}",
            );
        }
    }

    // ─── state_path ─────────────────────────────────────────────────────────

    #[test]
    fn state_path_lives_inside_config_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        let p = state_path(&env).expect("path resolves");
        assert_eq!(p, tmp.path().join(".notickets").join(STATE_FILE));
    }

    #[test]
    fn state_file_filename_pin() {
        assert_eq!(STATE_FILE, "state.json");
    }

    // ─── StateError Display ─────────────────────────────────────────────────

    #[test]
    fn state_error_display_home_unresolvable_names_the_problem() {
        let s = format!("{}", StateError::HomeUnresolvable);
        assert!(
            s.contains("config directory"),
            "must mention config directory; got {s:?}",
        );
    }

    #[test]
    fn state_error_display_io_includes_inner_error() {
        let inner = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "kaboom");
        let s = format!("{}", StateError::Io(inner));
        assert!(s.contains("state.json"), "must mention the file; got {s:?}",);
        assert!(
            s.contains("kaboom"),
            "must surface the underlying io error; got {s:?}",
        );
    }

    #[test]
    fn state_error_display_json_includes_parse_failure() {
        let inner = serde_json::from_str::<State>("not json").unwrap_err();
        let s = format!("{}", StateError::Json(inner));
        assert!(s.contains("state.json"), "must mention the file; got {s:?}",);
        assert!(s.contains("parse"), "must say `parse`; got {s:?}");
    }
}
