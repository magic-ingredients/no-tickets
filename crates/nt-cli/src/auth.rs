//! Auth resolution. NO_TICKETS_TOKEN env var beats the credentials file.
//! Mirrors `src/sdk/auth.ts::resolveAuth`.

use crate::credentials;
use crate::env::Env;

pub const NOT_AUTH_MSG: &str =
    "Not authenticated. Set NO_TICKETS_TOKEN or run `npx no-tickets init`.";

#[derive(Clone, Copy)]
pub enum AuthSource {
    Env,
    Credentials,
}

impl AuthSource {
    pub fn as_str(self) -> &'static str {
        match self {
            AuthSource::Env => "env",
            AuthSource::Credentials => "credentials",
        }
    }
}

#[derive(Clone, Copy)]
pub enum TokenType {
    Push,
    Session,
    Unknown,
}

impl TokenType {
    pub fn detect(token: &str) -> Self {
        if token.starts_with("nt_push_") {
            TokenType::Push
        } else if token.starts_with("nt_session_") {
            TokenType::Session
        } else {
            TokenType::Unknown
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            TokenType::Push => "push",
            TokenType::Session => "session",
            TokenType::Unknown => "unknown",
        }
    }
}

pub struct ResolvedAuth {
    /// The actual bearer token. Required by transport callers (publish);
    /// status doesn't read it (only displays source + tokenType).
    pub token: String,
    pub source: AuthSource,
    pub token_type: TokenType,
}

pub fn resolve_auth(env: &dyn Env) -> Option<ResolvedAuth> {
    // RED: signature accepts env but body still reads process env.
    // GREEN replaces std::env::var with env.var.
    let _ = env;
    if let Ok(token) = std::env::var("NO_TICKETS_TOKEN") {
        if !token.is_empty() {
            let token_type = TokenType::detect(&token);
            return Some(ResolvedAuth {
                token,
                source: AuthSource::Env,
                token_type,
            });
        }
    }
    let stored = credentials::load(env)?;
    let token_type = TokenType::detect(&stored.token);
    Some(ResolvedAuth {
        token: stored.token,
        source: AuthSource::Credentials,
        token_type,
    })
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

    #[test]
    fn resolve_auth_reads_token_from_injected_env() {
        let env = HashMapEnv::with(&[("NO_TICKETS_TOKEN", TEST_TOKEN)]);
        let resolved = resolve_auth(&env).expect("token resolved");
        assert_eq!(
            resolved.token, TEST_TOKEN,
            "resolve_auth must read NO_TICKETS_TOKEN from the injected env",
        );
        assert!(matches!(resolved.source, AuthSource::Env));
    }

    #[test]
    fn resolve_auth_detects_push_token_type_from_injected_env() {
        let env = HashMapEnv::with(&[("NO_TICKETS_TOKEN", "nt_push_xyz")]);
        let resolved = resolve_auth(&env).expect("token resolved");
        assert!(matches!(resolved.token_type, TokenType::Push));
    }

    #[test]
    fn resolve_auth_detects_session_token_type_from_injected_env() {
        let env = HashMapEnv::with(&[("NO_TICKETS_TOKEN", "nt_session_abc")]);
        let resolved = resolve_auth(&env).expect("token resolved");
        assert!(matches!(resolved.token_type, TokenType::Session));
    }
}
