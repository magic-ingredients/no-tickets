//! Auth resolution. NO_TICKETS_TOKEN env var beats the credentials file.
//! Mirrors `src/sdk/auth.ts::resolveAuth`.
//!
//! Per ADR-0002, session credentials from disk carry a `host` tag — the
//! api_url they were issued against. `resolve_auth` takes the current
//! api_url so it can surface a host-mismatch warning instead of silently
//! treating a stale session as authenticated.

use crate::credentials::{self, LoadOutcome};
use crate::env::Env;

pub const NOT_AUTH_MSG: &str = "Not authenticated. Set NO_TICKETS_TOKEN or run `no-tickets init`.";

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
    /// The actual bearer token. Required by transport callers (publish).
    pub token: String,
    pub source: AuthSource,
    /// Set when `source == Credentials`. Surfaced by `no-tickets status` as the
    /// identity attached to an authenticated session. None for env-supplied
    /// `NO_TICKETS_TOKEN` — those are transport-level overrides, not
    /// identity claims.
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

pub fn resolve_auth(env: &dyn Env, current_api_url: &str) -> AuthOutcome {
    if let Some(token) = env.var("NO_TICKETS_TOKEN") {
        if !token.is_empty() {
            return AuthOutcome::Resolved(ResolvedAuth {
                token,
                source: AuthSource::Env,
                email: None,
            });
        }
    }
    match credentials::load(env, current_api_url) {
        LoadOutcome::Valid(stored) => AuthOutcome::Resolved(ResolvedAuth {
            token: stored.token,
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
    fn resolve_auth_reads_token_from_injected_env() {
        let env = HashMapEnv::with(&[("NO_TICKETS_TOKEN", TEST_TOKEN)]);
        let outcome = resolve_auth(&env, API_URL_PROD);
        match outcome {
            AuthOutcome::Resolved(r) => {
                assert_eq!(
                    r.token, TEST_TOKEN,
                    "resolve_auth must read NO_TICKETS_TOKEN from the injected env",
                );
                assert!(matches!(r.source, AuthSource::Env));
            }
            _ => panic!("expected Resolved; got something else"),
        }
    }

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
