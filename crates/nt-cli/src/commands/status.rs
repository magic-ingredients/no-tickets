//! `nt status` command: resolve URLs, resolve auth, print JSON to stdout.
//! Mirrors `src/cli.ts::handleStatus`.

use std::io::{self, Write};

use serde::Serialize;

use crate::auth::{resolve_auth, ResolvedAuth, NOT_AUTH_MSG};
use crate::env::Env;
use crate::urls::{resolve_urls, ResolvedUrls};

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

/// Pure builder for the authenticated status JSON payload.
///
/// **Precondition (encoded in the name):** caller has already established
/// the user is authenticated. `run()` short-circuits with a stderr
/// message + non-zero exit if auth resolution fails, so this builder is
/// only ever reached on the happy path. The hardcoded `authenticated: true`
/// reflects that invariant; the name signals it to readers.
///
/// `urls` is taken by value (moved) rather than by reference — `run()`
/// has no further use for it and the alternative requires two String
/// clones per invocation. `auth` stays by reference since we only read
/// its enum tags (no allocation).
///
/// Pure: no I/O, no env reads, no time. Field order on the wire is
/// pinned by StatusOutput's declaration order (serde_derive emits in
/// declaration order) — matches the TS `handleStatus` object literal.
fn build_authenticated_output(auth: &ResolvedAuth, urls: ResolvedUrls) -> StatusOutput {
    StatusOutput {
        authenticated: true,
        source: auth.source.as_str(),
        token_type: auth.token_type.as_str(),
        api_url: urls.api_url,
        auth_url: urls.auth_url,
    }
}

pub fn run(env: &dyn Env) -> i32 {
    // URL resolution runs before auth resolution — matches TS handleStatus,
    // where urlsForFlagsOrFail returns before describeAuthStatus is called.
    // This is what makes URL-error scenarios surface even when
    // NO_TICKETS_TOKEN is set (URL error wins).
    let urls = match resolve_urls(env) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("{}", e.user_message());
            return 1;
        }
    };

    let Some(auth) = resolve_auth(env) else {
        eprintln!("{NOT_AUTH_MSG}");
        return 1;
    };

    let out = build_authenticated_output(&auth, urls);
    let json = serde_json::to_string(&out).expect("status payload serializes");
    let stdout = io::stdout();
    let write_result = writeln!(stdout.lock(), "{json}");
    write_result_to_exit_code(write_result)
}

/// Pure mapping from a stdout-write outcome to the process exit code.
///
/// Broken-pipe (stdout closed by a consumer — `| head -n 1`, etc.) is a
/// normal exit; the consumer signalled "I have enough" and the kernel
/// closed the read end of the pipe. Anything else (disk full, hardware
/// I/O failure, permissions) is a hard failure that the caller should
/// be told about via a non-zero exit.
///
/// Pure: input is the `io::Result` from a write, output is the exit code.
/// Tested with fakes for both error kinds; the integration test
/// `status_broken_pipe_on_stdout_exits_zero` exercises the
/// real-process broken-pipe path end-to-end.
fn write_result_to_exit_code(result: io::Result<()>) -> i32 {
    match result {
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
            token: "unused-by-builder".to_string(),
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
    fn build_authenticated_output_field_order_matches_ts_object_literal() {
        let auth = sample_auth(AuthSource::Env, TokenType::Push);
        let out = build_authenticated_output(&auth, sample_urls());
        let body = serde_json::to_string(&out).expect("serialises");
        let a = body
            .find(r#""authenticated":"#)
            .expect("authenticated present");
        let s = body.find(r#""source":"#).expect("source present");
        let tt = body.find(r#""tokenType":"#).expect("tokenType present");
        let au = body.find(r#""apiUrl":"#).expect("apiUrl present");
        let aurl = body.find(r#""authUrl":"#).expect("authUrl present");
        assert!(
            a < s && s < tt && tt < au && au < aurl,
            "field order must match TS object literal; got {body}",
        );
    }

    #[test]
    fn build_authenticated_output_always_emits_authenticated_true() {
        // Precondition: builder only runs on the authenticated path
        // (run() short-circuits otherwise). The hardcoded `true` is
        // part of the contract — pinned here so a future refactor
        // that adds a bool parameter has to delete this test
        // deliberately.
        let auth = sample_auth(AuthSource::Env, TokenType::Push);
        let out = build_authenticated_output(&auth, sample_urls());
        let body = serde_json::to_string(&out).expect("serialises");
        assert!(body.contains(r#""authenticated":true"#));
    }

    /// Table-driven coverage of every AuthSource × TokenType variant.
    /// Consolidates what were six near-identical single-variant tests;
    /// each row pins one variant to its expected wire string and the
    /// table forces exhaustiveness when new variants are added.
    #[test]
    fn build_authenticated_output_renders_every_auth_source_and_token_type_variant() {
        let source_cases: &[(AuthSource, &str)] = &[
            (AuthSource::Env, r#""source":"env""#),
            (AuthSource::Credentials, r#""source":"credentials""#),
        ];
        let token_cases: &[(TokenType, &str)] = &[
            (TokenType::Push, r#""tokenType":"push""#),
            (TokenType::Session, r#""tokenType":"session""#),
            (TokenType::Unknown, r#""tokenType":"unknown""#),
        ];

        for (source_variant, expected_source) in source_cases {
            for (token_variant, expected_token) in token_cases {
                let auth = sample_auth(*source_variant, *token_variant);
                let out = build_authenticated_output(&auth, sample_urls());
                let body = serde_json::to_string(&out).expect("serialises");
                assert!(
                    body.contains(expected_source),
                    "expected {expected_source:?} for source variant rendered as {:?}; got {body}",
                    source_variant.as_str(),
                );
                assert!(
                    body.contains(expected_token),
                    "expected {expected_token:?} for token variant rendered as {:?}; got {body}",
                    token_variant.as_str(),
                );
            }
        }
    }

    #[test]
    fn build_authenticated_output_passes_through_api_and_auth_urls() {
        let auth = sample_auth(AuthSource::Env, TokenType::Push);
        let urls = ResolvedUrls {
            api_url: "https://staging-api.no-tickets.com".to_string(),
            auth_url: "https://staging.no-tickets.com/api/auth/cli".to_string(),
        };
        let out = build_authenticated_output(&auth, urls);
        let body = serde_json::to_string(&out).expect("serialises");
        assert!(body.contains(r#""apiUrl":"https://staging-api.no-tickets.com""#));
        assert!(body.contains(r#""authUrl":"https://staging.no-tickets.com/api/auth/cli""#));
    }

    #[test]
    fn write_result_ok_maps_to_exit_zero() {
        assert_eq!(write_result_to_exit_code(Ok(())), 0);
    }

    #[test]
    fn write_result_broken_pipe_maps_to_exit_zero() {
        // BrokenPipe = consumer closed the read end (`| head -n 1` etc.).
        // Normal termination, not a failure.
        let err = io::Error::new(io::ErrorKind::BrokenPipe, "pipe closed");
        assert_eq!(write_result_to_exit_code(Err(err)), 0);
    }

    #[test]
    fn write_result_non_broken_pipe_error_maps_to_exit_one() {
        // Any stdout error that is NOT BrokenPipe is a hard failure.
        // Pins the asymmetry of the match guard — mutation testing
        // surfaced this branch's lack of coverage.
        let err = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        assert_eq!(write_result_to_exit_code(Err(err)), 1);
    }

    #[test]
    fn write_result_other_error_kind_also_maps_to_exit_one() {
        // Belt-and-braces: a different non-BrokenPipe kind also exits 1.
        // Confirms the guard discriminates on BrokenPipe specifically,
        // not on "some particular error" / "is_some" / etc.
        let err = io::Error::other("weird");
        assert_eq!(write_result_to_exit_code(Err(err)), 1);
    }
}
