//! Auth resolution.
//!
//! Two distinct resolvers, by purpose:
//!
//! - **`resolve_publish_token`** — what `nt publish` uses. Reads the
//!   push token registered for the caller-supplied `--project` from
//!   `config.json` (or `NO_TICKETS_TOKEN` as a CI escape hatch).
//!   Session credentials from `nt init` are NEVER consulted here —
//!   they're a management-API identity, not a publish credential. See
//!   `docs/fixes/publish-uses-push-token.md`.
//!
//! - **`resolve_auth`** — what identity / management commands use
//!   (`nt status` today; future identity commands). Reads the session
//!   credentials file written by `nt init`, with the env-var escape
//!   hatch first. Carries the host-tag check from ADR-0002 so a stale
//!   session (different env than the one currently selected) surfaces
//!   as `SessionHostMismatch` rather than silently treating a stale
//!   session as authenticated.

use crate::config;
use crate::credentials::{self, LoadOutcome};
use crate::env::Env;
use crate::error::NtError;

/// Emits the ADR-0002 stored-session host-mismatch warning to stderr.
/// Centralised so identity-aware callers (status, publish, future commands)
/// share one phrasing. Token is never included — the warning is identity-free.
pub fn emit_host_mismatch_warning(stored_host: &str, current_host: &str) {
    eprintln!(
        "Warning: stored session was issued for {stored_host} but the current environment resolves to {current_host}. Run `no-tickets init` to re-authenticate against the current environment.",
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AuthSource {
    /// Token came from `NO_TICKETS_TOKEN` env var. Transport-level escape
    /// hatch; doesn't count as authenticated identity in `no-tickets status`.
    Env,
    /// Token came from the session credentials file. The
    /// "authenticated" identity in `no-tickets status`.
    Credentials,
}

pub struct ResolvedAuth {
    pub source: AuthSource,
    /// Set when `source == Credentials`. Surfaced by `no-tickets status` as
    /// the identity attached to an authenticated session. `None` for
    /// env-supplied `NO_TICKETS_TOKEN` — those are transport-level
    /// overrides, not identity claims.
    pub email: Option<String>,
}

/// What `resolve_auth` reports back to the caller.
///
/// `SessionHostMismatch` is the new branch added by ADR-0002 Task 3: when
/// the credentials file's stored `host` doesn't match the caller's
/// `current_api_url`, callers (status, publish, future identity commands)
/// must warn the user and decline to use the stored session.
pub enum AuthOutcome {
    Resolved(ResolvedAuth),
    None,
    SessionHostMismatch {
        stored_host: String,
        current_host: String,
    },
}

/// Resolve the Bearer token `nt publish` will send for the given
/// `--project`. Either:
///
/// - `NO_TICKETS_TOKEN` env var (CI escape hatch, wins when non-empty), or
/// - `config.json`'s `projects[project].pushToken` (the canonical path,
///   written by `nt token add`).
///
/// Session credentials from `nt init` are deliberately NOT consulted.
/// Session tokens are a management-API identity claim and must not
/// reach `/v1/events` (privilege confusion). Unregistered projects
/// surface as `NtError::ProjectNotRegistered` (exit code 6) — sharp
/// signal carrying the offending project name AND the
/// locally-registered alternatives so wrappers can offer a hint.
///
/// Config-read failures (malformed JSON, I/O error, unresolvable home)
/// surface as `NtError::Usage` since they're caller-environment problems
/// rather than auth-state problems.
pub fn resolve_publish_token(env: &dyn Env, project: &str) -> Result<String, NtError> {
    // `.trim().is_empty()` rather than `.is_empty()`: a whitespace-only
    // NO_TICKETS_TOKEN is almost certainly a misconfiguration (rendered
    // env-var template with a missing value, accidental quote-wrapped
    // empty string in CI). Treat it as unset rather than as a bearer.
    if let Some(token) = env.var("NO_TICKETS_TOKEN").filter(|t| !t.trim().is_empty()) {
        return Ok(token);
    }
    // ConfigError's `Display` already carries the variant context
    // (HomeUnresolvable / Io / Json) so we just propagate it verbatim.
    // Mapping to `NtError::Usage` reflects the cause: a caller-side
    // environment problem (malformed file / missing config dir), not
    // an auth-state failure.
    let cfg = config::read(env).map_err(|e| NtError::Usage {
        message: format!("{e}"),
    })?;
    if let Some(entry) = cfg.projects.get(project) {
        return Ok(entry.push_token.clone());
    }
    Err(NtError::ProjectNotRegistered {
        project: project.to_string(),
        known_projects: cfg.projects.keys().cloned().collect(),
    })
}

pub fn resolve_auth(env: &dyn Env, current_api_url: &str) -> AuthOutcome {
    if env
        .var("NO_TICKETS_TOKEN")
        .is_some_and(|t| !t.trim().is_empty())
    {
        return AuthOutcome::Resolved(ResolvedAuth {
            source: AuthSource::Env,
            email: None,
        });
    }
    match credentials::load(env, current_api_url) {
        LoadOutcome::Valid(stored) => AuthOutcome::Resolved(ResolvedAuth {
            source: AuthSource::Credentials,
            email: Some(stored.email),
        }),
        LoadOutcome::HostMismatch { stored_host } => AuthOutcome::SessionHostMismatch {
            stored_host,
            current_host: current_api_url.to_string(),
        },
        LoadOutcome::None => AuthOutcome::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::HashMapEnv;

    /// Distinctive sentinel that cannot collide with real shell state.
    /// If a test asserts `== TEST_TOKEN` and reality returns anything else
    /// (default, host env value, None), the test fails — which is what
    /// drives the RED→GREEN transition.
    const TEST_TOKEN: &str = "nt_push_red_phase_sentinel_z9q3";
    const API_URL_PROD: &str = "https://api.no-tickets.com";
    const API_URL_STAGING: &str = "https://api-staging.no-tickets.com";

    #[test]
    fn resolve_auth_classifies_injected_env_token_as_env_source() {
        let env = HashMapEnv::with(&[("NO_TICKETS_TOKEN", TEST_TOKEN)]);
        let outcome = resolve_auth(&env, API_URL_PROD);
        match outcome {
            AuthOutcome::Resolved(r) => {
                assert!(matches!(r.source, AuthSource::Env));
                assert_eq!(r.email, None, "env-supplied tokens carry no identity");
            }
            _ => panic!("expected Resolved; got something else"),
        }
    }

    #[test]
    fn resolve_auth_treats_whitespace_only_env_var_as_unset() {
        // Point NO_TICKETS_HOME at an empty tempdir so credentials::load
        // can't fall back to the host's real ~/.notickets when the
        // whitespace env-var is correctly rejected. Without this the
        // test fails non-hermetically on machines with real session
        // credentials on disk.
        let tmp = tempfile::tempdir().unwrap();
        let env = HashMapEnv::with(&[
            ("NO_TICKETS_HOME", tmp.path().to_str().unwrap()),
            ("NO_TICKETS_TOKEN", "   \t  "),
        ]);
        let outcome = resolve_auth(&env, API_URL_PROD);
        assert!(matches!(outcome, AuthOutcome::None));
    }

    // ─── resolve_publish_token ────────────────────────────────────────

    /// Write a config.json under `$home/.notickets/` with a single project.
    fn write_test_config(home: &std::path::Path, project: &str, push_token: &str) {
        let dir = home.join(".notickets");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.json"),
            format!(
                r#"{{"projects":{{"{project}":{{"pushToken":"{push_token}","addedAt":"2026-05-20T00:00:00.000Z"}}}}}}"#,
            ),
        )
        .unwrap();
    }

    #[test]
    fn resolve_publish_token_reads_env_var_first_when_set() {
        let tmp = tempfile::tempdir().unwrap();
        // Config.json has a different token; env var must still win.
        write_test_config(tmp.path(), "demo", "nt_push_from_config");
        let env = HashMapEnv::with(&[
            ("NO_TICKETS_HOME", tmp.path().to_str().unwrap()),
            ("NO_TICKETS_TOKEN", TEST_TOKEN),
        ]);
        let token = resolve_publish_token(&env, "demo").expect("resolves");
        assert_eq!(token, TEST_TOKEN);
    }

    #[test]
    fn resolve_publish_token_skips_empty_env_var_and_falls_through_to_config() {
        let tmp = tempfile::tempdir().unwrap();
        write_test_config(tmp.path(), "demo", "nt_push_from_config");
        let env = HashMapEnv::with(&[
            ("NO_TICKETS_HOME", tmp.path().to_str().unwrap()),
            ("NO_TICKETS_TOKEN", ""), // empty must NOT win
        ]);
        let token = resolve_publish_token(&env, "demo").expect("resolves");
        assert_eq!(token, "nt_push_from_config");
    }

    #[test]
    fn resolve_publish_token_treats_whitespace_only_env_var_as_unset() {
        // Whitespace-only NO_TICKETS_TOKEN must NOT be sent as a
        // bearer — it's almost always a CI misconfiguration (rendered
        // env-var template with a missing value). Behave as if unset
        // and fall through to the config.json registry.
        let tmp = tempfile::tempdir().unwrap();
        write_test_config(tmp.path(), "demo", "nt_push_from_config");
        let env = HashMapEnv::with(&[
            ("NO_TICKETS_HOME", tmp.path().to_str().unwrap()),
            ("NO_TICKETS_TOKEN", "   \t  "),
        ]);
        let token = resolve_publish_token(&env, "demo").expect("resolves");
        assert_eq!(token, "nt_push_from_config");
    }

    #[test]
    fn resolve_publish_token_reads_config_json_when_no_env_var() {
        let tmp = tempfile::tempdir().unwrap();
        write_test_config(tmp.path(), "demo", "nt_push_from_config");
        let env = HashMapEnv::with(&[("NO_TICKETS_HOME", tmp.path().to_str().unwrap())]);
        let token = resolve_publish_token(&env, "demo").expect("resolves");
        assert_eq!(token, "nt_push_from_config");
    }

    #[test]
    fn resolve_publish_token_errors_with_project_not_registered_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        // No config.json at all. Project is unregistered.
        let env = HashMapEnv::with(&[("NO_TICKETS_HOME", tmp.path().to_str().unwrap())]);
        let err = resolve_publish_token(&env, "demo").expect_err("must error");
        match err {
            NtError::ProjectNotRegistered {
                project,
                known_projects,
            } => {
                assert_eq!(project, "demo");
                assert!(
                    known_projects.is_empty(),
                    "empty config → no known projects; got {known_projects:?}",
                );
            }
            other => panic!("expected ProjectNotRegistered; got {other:?}"),
        }
    }

    #[test]
    fn resolve_publish_token_lists_known_projects_when_some_are_registered() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".notickets");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.json"),
            r#"{"projects":{"other-a":{"pushToken":"nt_push_a","addedAt":"2026-05-20T00:00:00.000Z"},"other-b":{"pushToken":"nt_push_b","addedAt":"2026-05-20T00:00:00.000Z"}}}"#,
        )
        .unwrap();
        let env = HashMapEnv::with(&[("NO_TICKETS_HOME", tmp.path().to_str().unwrap())]);

        let err = resolve_publish_token(&env, "demo").expect_err("must error");
        match err {
            NtError::ProjectNotRegistered {
                project,
                known_projects,
            } => {
                assert_eq!(project, "demo");
                // Assert by set membership rather than vec equality so
                // the test doesn't bake in BTreeMap iteration order
                // (currently alpha, but treating that as a load-bearing
                // contract is a footgun for a future map-impl swap).
                let names: std::collections::HashSet<&str> =
                    known_projects.iter().map(|s| s.as_str()).collect();
                assert_eq!(
                    names,
                    ["other-a", "other-b"].into_iter().collect(),
                    "knownProjects must contain exactly the registered names; got: {known_projects:?}",
                );
            }
            other => panic!("expected ProjectNotRegistered; got {other:?}"),
        }
    }

    #[test]
    fn resolve_publish_token_surfaces_malformed_config_as_usage_error() {
        // A corrupted config.json (truncated JSON, manual edit gone
        // wrong, etc.) must surface as `NtError::Usage` (caller-env
        // problem) — NOT silently masked as "no projects registered"
        // which would mislead the user into running `token add` again
        // when the real issue is a parse error they need to fix.
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".notickets");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.json"), "{ not valid json").unwrap();
        let env = HashMapEnv::with(&[("NO_TICKETS_HOME", tmp.path().to_str().unwrap())]);

        let err = resolve_publish_token(&env, "demo").expect_err("must error");
        match err {
            NtError::Usage { message } => {
                assert!(
                    message.to_lowercase().contains("config"),
                    "Usage message must name config; got: {message:?}",
                );
            }
            other => panic!("expected Usage; got {other:?}"),
        }
    }

    #[test]
    fn resolve_publish_token_does_not_consult_session_credentials_file() {
        // Architectural pin: a perfectly valid credentials file on disk
        // matching the same env must NOT be consulted by the publish
        // token resolver. The session token never reaches /v1/events.
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".notickets");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("credentials"),
            format!(
                r#"{{"token":"nt_session_NEVER_USED","email":"a@b.com","expiresAt":"2099-01-01T00:00:00.000Z","host":"{API_URL_PROD}"}}"#,
            ),
        )
        .unwrap();
        let env = HashMapEnv::with(&[("NO_TICKETS_HOME", tmp.path().to_str().unwrap())]);

        // No env var, no config.json — only the credentials file. Must
        // surface as ProjectNotRegistered, not as a session-token leak.
        let err = resolve_publish_token(&env, "demo").expect_err("must error");
        assert!(matches!(err, NtError::ProjectNotRegistered { .. }));
    }

    // ─── resolve_auth (identity/status) ────────────────────────────────

    #[test]
    fn resolve_auth_returns_session_host_mismatch_when_credentials_host_differs_from_current() {
        // Write a credentials file with host=staging; ask for prod.
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".notickets");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("credentials"),
            format!(
                r#"{{"token":"nt_session_x","email":"a@b.com","expiresAt":"2099-01-01T00:00:00.000Z","host":"{API_URL_STAGING}"}}"#,
            ),
        )
        .unwrap();
        let env = HashMapEnv::with(&[("NO_TICKETS_HOME", tmp.path().to_str().unwrap())]);

        let outcome = resolve_auth(&env, API_URL_PROD);
        match outcome {
            AuthOutcome::SessionHostMismatch {
                stored_host,
                current_host,
            } => {
                assert_eq!(stored_host, API_URL_STAGING);
                assert_eq!(current_host, API_URL_PROD);
            }
            _ => panic!("expected SessionHostMismatch; got something else"),
        }
    }
}
