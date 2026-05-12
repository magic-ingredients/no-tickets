//! `nt token list` — JSON of locally-registered push tokens.
//!
//! Output shape: `{ "tokens": [{ "project", "masked", "addedAt", "label"? }, …] }`.
//! Entries are emitted in `BTreeMap` (lexicographic) order — deterministic
//! for piping into `jq` and snapshot tests.

use serde::Serialize;

use crate::config;
use crate::env::Env;

#[derive(Serialize)]
struct Output {
    tokens: Vec<TokenEntry>,
}

#[derive(Serialize)]
struct TokenEntry {
    project: String,
    masked: String,
    #[serde(rename = "addedAt")]
    added_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
}

pub fn run(env: &dyn Env) -> i32 {
    let cfg = match config::read(env) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let tokens = cfg
        .projects
        .into_iter()
        .map(|(project, entry)| TokenEntry {
            project,
            masked: config::mask_token(&entry.push_token),
            added_at: entry.added_at,
            label: entry.label,
        })
        .collect();
    let body = serde_json::to_string(&Output { tokens }).expect("token list payload serialises");
    println!("{body}");
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

    fn list_to_string(env: &dyn Env) -> String {
        let cfg = config::read(env).expect("read");
        let tokens: Vec<TokenEntry> = cfg
            .projects
            .into_iter()
            .map(|(project, entry)| TokenEntry {
                project,
                masked: config::mask_token(&entry.push_token),
                added_at: entry.added_at,
                label: entry.label,
            })
            .collect();
        serde_json::to_string(&Output { tokens }).expect("serialises")
    }

    #[test]
    fn token_list_returns_empty_array_when_no_projects_registered() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        let body = list_to_string(&env);
        assert_eq!(body, r#"{"tokens":[]}"#);
    }

    #[test]
    fn token_list_emits_project_masked_addedat_label_for_each_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        assert_eq!(
            token_add::run(&env, "alpha", "nt_push_abcdefgh1111", Some("dev"), false),
            0
        );
        assert_eq!(
            token_add::run(&env, "beta", "nt_push_abcdefgh2222", None, false),
            0
        );
        let body = list_to_string(&env);
        // Entries appear in lexicographic order from BTreeMap.
        let alpha_pos = body.find(r#""project":"alpha""#).expect("alpha");
        let beta_pos = body.find(r#""project":"beta""#).expect("beta");
        assert!(alpha_pos < beta_pos, "order: alpha before beta; got {body}");

        // Masked, not raw.
        assert!(body.contains(r#""masked":"nt_push_…1111""#), "got {body}");
        assert!(body.contains(r#""masked":"nt_push_…2222""#), "got {body}");
        // Raw token MUST NOT appear in the list output.
        assert!(!body.contains("abcdefgh"), "raw token leaked: {body}");

        // Label present on alpha, omitted on beta.
        assert!(body.contains(r#""label":"dev""#));
        assert!(
            !body.contains(r#""project":"beta","masked":"nt_push_…2222","addedAt":"...","label":"#),
            "no label key on beta when absent"
        );
    }
}
