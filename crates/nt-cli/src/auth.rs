//! Auth resolution. NO_TICKETS_TOKEN env var beats the credentials file.
//! Mirrors `src/sdk/auth.ts::resolveAuth`.

use crate::credentials;

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

pub fn resolve_auth() -> Option<ResolvedAuth> {
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
    let stored = credentials::load()?;
    let token_type = TokenType::detect(&stored.token);
    Some(ResolvedAuth {
        token: stored.token,
        source: AuthSource::Credentials,
        token_type,
    })
}
