//! `nt status` command: resolve URLs, resolve auth, print JSON to stdout.
//! Mirrors `src/cli.ts::handleStatus`.

use std::io::{self, Write};

use serde::Serialize;

use crate::auth::{NOT_AUTH_MSG, ResolvedAuth, resolve_auth};
use crate::urls::{ResolvedUrls, resolve_urls};

/// Field order MUST match the TS object literal:
/// `{ authenticated, source, tokenType, apiUrl, authUrl }`. serde_derive
/// emits in declaration order, so don't reorder these.
#[derive(Serialize)]
struct StatusOutput {
    authenticated: bool,
    source: &'static str,
    #[serde(rename = "tokenType")]
    token_type: &'static str,
    #[serde(rename = "apiUrl")]
    api_url: String,
    #[serde(rename = "authUrl")]
    auth_url: String,
}

/// Pure builder for the status JSON payload. Stub for RED phase; GREEN
/// extracts the literal from `run()` and replaces this body.
fn build_output(_auth: &ResolvedAuth, _urls: &ResolvedUrls) -> StatusOutput {
    unimplemented!("build_output: extracted in GREEN phase")
}

pub fn run(profile: Option<&str>) -> i32 {
    // URL resolution runs before auth resolution — matches TS handleStatus,
    // where urlsForFlagsOrFail returns before describeAuthStatus is called.
    // This is what makes the profile-error tests work even when
    // NO_TICKETS_TOKEN is set (URL error wins).
    let urls = match resolve_urls(profile) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("{}", e.user_message());
            return 1;
        }
    };

    let Some(auth) = resolve_auth() else {
        eprintln!("{NOT_AUTH_MSG}");
        return 1;
    };

    let out = StatusOutput {
        authenticated: true,
        source: auth.source.as_str(),
        token_type: auth.token_type.as_str(),
        api_url: urls.api_url,
        auth_url: urls.auth_url,
    };
    let json = serde_json::to_string(&out).expect("status payload serializes");
    // Broken-pipe (stdout closed by consumer — `| head -n 1`, etc.) is a
    // normal exit, not a panic. Anything else from stdout is a hard failure.
    let stdout = io::stdout();
    match writeln!(stdout.lock(), "{json}") {
        Ok(()) => 0,
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => 0,
        Err(_) => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AuthSource, TokenType};

    fn sample_auth(source: AuthSource, token_type: TokenType) -> ResolvedAuth {
        ResolvedAuth {
            token: "ignored-by-build_output".to_string(),
            source,
            token_type,
        }
    }

    fn sample_urls() -> ResolvedUrls {
        ResolvedUrls {
            api_url: "https://api.example.test".to_string(),
            auth_url: "https://app.example.test/api/auth/cli".to_string(),
        }
    }

    #[test]
    fn build_output_field_order_is_authenticated_source_tokentype_apiurl_authurl() {
        let auth = sample_auth(AuthSource::Env, TokenType::Push);
        let urls = sample_urls();
        let out = build_output(&auth, &urls);
        let body = serde_json::to_string(&out).expect("serialises");
        let a = body.find(r#""authenticated":"#).expect("authenticated present");
        let s = body.find(r#""source":"#).expect("source present");
        let tt = body.find(r#""tokenType":"#).expect("tokenType present");
        let au = body.find(r#""apiUrl":"#).expect("apiUrl present");
        let urlk = body.find(r#""authUrl":"#).expect("authUrl present");
        assert!(
            a < s && s < tt && tt < au && au < urlk,
            "field order must match TS object literal; got {body}",
        );
    }

    #[test]
    fn build_output_authenticated_is_true() {
        let auth = sample_auth(AuthSource::Env, TokenType::Push);
        let urls = sample_urls();
        let out = build_output(&auth, &urls);
        let body = serde_json::to_string(&out).expect("serialises");
        assert!(body.contains(r#""authenticated":true"#));
    }

    #[test]
    fn build_output_auth_source_env_renders_as_env() {
        let auth = sample_auth(AuthSource::Env, TokenType::Unknown);
        let urls = sample_urls();
        let out = build_output(&auth, &urls);
        let body = serde_json::to_string(&out).expect("serialises");
        assert!(body.contains(r#""source":"env""#), "got {body}");
    }

    #[test]
    fn build_output_auth_source_credentials_renders_as_credentials() {
        let auth = sample_auth(AuthSource::Credentials, TokenType::Unknown);
        let urls = sample_urls();
        let out = build_output(&auth, &urls);
        let body = serde_json::to_string(&out).expect("serialises");
        assert!(body.contains(r#""source":"credentials""#), "got {body}");
    }

    #[test]
    fn build_output_token_type_push_renders_as_push() {
        let auth = sample_auth(AuthSource::Env, TokenType::Push);
        let urls = sample_urls();
        let out = build_output(&auth, &urls);
        let body = serde_json::to_string(&out).expect("serialises");
        assert!(body.contains(r#""tokenType":"push""#), "got {body}");
    }

    #[test]
    fn build_output_token_type_session_renders_as_session() {
        let auth = sample_auth(AuthSource::Env, TokenType::Session);
        let urls = sample_urls();
        let out = build_output(&auth, &urls);
        let body = serde_json::to_string(&out).expect("serialises");
        assert!(body.contains(r#""tokenType":"session""#), "got {body}");
    }

    #[test]
    fn build_output_token_type_unknown_renders_as_unknown() {
        let auth = sample_auth(AuthSource::Env, TokenType::Unknown);
        let urls = sample_urls();
        let out = build_output(&auth, &urls);
        let body = serde_json::to_string(&out).expect("serialises");
        assert!(body.contains(r#""tokenType":"unknown""#), "got {body}");
    }

    #[test]
    fn build_output_passes_through_api_and_auth_urls() {
        let auth = sample_auth(AuthSource::Env, TokenType::Push);
        let urls = ResolvedUrls {
            api_url: "https://staging-api.no-tickets.com".to_string(),
            auth_url: "https://staging.no-tickets.com/api/auth/cli".to_string(),
        };
        let out = build_output(&auth, &urls);
        let body = serde_json::to_string(&out).expect("serialises");
        assert!(body.contains(r#""apiUrl":"https://staging-api.no-tickets.com""#));
        assert!(body.contains(r#""authUrl":"https://staging.no-tickets.com/api/auth/cli""#));
    }
}
