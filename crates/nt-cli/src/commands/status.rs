//! `nt status` — JSON of session identity + locally-registered push tokens.
//!
//! Per ADR-0002, the output shape is:
//!
//! ```json
//! { "authenticated": false, "tokens": [] }
//! { "authenticated": false, "tokens": [{project, masked, addedAt, label?}, …] }
//! { "authenticated": true,  "email": "x@y.com", "tokens": […] }
//! ```
//!
//! Plus, on session-host mismatch, a `Warning:` line to stderr (the JSON
//! still emits `authenticated: false`, since the stored session was
//! declined).
//!
//! URL-resolution errors (partial pair, unknown preset, env+pair conflict)
//! surface to stderr with exit 1 — those are bad config, not auth state.

use std::io::{self, Write};

use serde::Serialize;

use crate::auth::{emit_host_mismatch_warning, resolve_auth, AuthOutcome, AuthSource};
use crate::config;
use crate::env::Env;
use crate::urls::resolve_urls;

#[derive(Serialize)]
struct StatusOutput {
    authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
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
    // Bad URL config wins over auth state — same precedence as the TS
    // reference. Pin: error-text variants surface to stderr; exit 1.
    let urls = match resolve_urls(env) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("{}", e.user_message());
            return 1;
        }
    };

    let tokens = load_token_entries(env);

    let (authenticated, email) = match resolve_auth(env, &urls.api_url) {
        AuthOutcome::Resolved(auth) => match auth.source {
            // Only the credentials-file path counts as "authenticated" in
            // the identity sense (ADR-0002). `NO_TICKETS_TOKEN` env is a
            // transport-level override that doesn't surface here.
            AuthSource::Credentials => (true, auth.email),
            AuthSource::Env => (false, None),
        },
        AuthOutcome::None => (false, None),
        AuthOutcome::SessionHostMismatch {
            stored_host,
            current_host,
        } => {
            emit_host_mismatch_warning(&stored_host, &current_host);
            (false, None)
        }
    };

    let out = StatusOutput {
        authenticated,
        email,
        tokens,
    };
    let json = serde_json::to_string(&out).expect("status payload serializes");
    let stdout = io::stdout();
    let write_result = writeln!(stdout.lock(), "{json}");
    write_result_to_exit_code(write_result)
}

/// Reads the local config registry. On any error (missing file, malformed
/// JSON) returns an empty list — status SHOULD render whatever it can
/// rather than fail just because the token registry is unreadable.
fn load_token_entries(env: &dyn Env) -> Vec<TokenEntry> {
    let Ok(cfg) = config::read(env) else {
        return Vec::new();
    };
    cfg.projects
        .into_iter()
        .map(|(project, entry)| TokenEntry {
            project,
            masked: config::mask_token(&entry.push_token),
            added_at: entry.added_at,
            label: entry.label,
        })
        .collect()
}

/// Pure mapping from a stdout-write outcome to the process exit code.
///
/// Broken-pipe (stdout closed by a consumer — `| head -n 1`, etc.) is a
/// normal exit; the consumer signalled "I have enough" and the kernel
/// closed the read end of the pipe. Anything else (disk full, hardware
/// I/O failure, permissions) is a hard failure that the caller should
/// be told about via a non-zero exit.
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

    #[test]
    fn write_result_ok_maps_to_exit_zero() {
        assert_eq!(write_result_to_exit_code(Ok(())), 0);
    }

    #[test]
    fn write_result_broken_pipe_maps_to_exit_zero() {
        let err = io::Error::new(io::ErrorKind::BrokenPipe, "pipe closed");
        assert_eq!(write_result_to_exit_code(Err(err)), 0);
    }

    #[test]
    fn write_result_non_broken_pipe_error_maps_to_exit_one() {
        let err = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        assert_eq!(write_result_to_exit_code(Err(err)), 1);
    }

    #[test]
    fn status_output_skips_email_when_unauthenticated() {
        let body = serde_json::to_string(&StatusOutput {
            authenticated: false,
            email: None,
            tokens: vec![],
        })
        .unwrap();
        assert!(body.contains(r#""authenticated":false"#));
        assert!(!body.contains(r#""email""#));
        assert!(body.contains(r#""tokens":[]"#));
    }

    #[test]
    fn status_output_includes_email_when_authenticated() {
        let body = serde_json::to_string(&StatusOutput {
            authenticated: true,
            email: Some("x@y.com".to_string()),
            tokens: vec![],
        })
        .unwrap();
        assert!(body.contains(r#""authenticated":true"#));
        assert!(body.contains(r#""email":"x@y.com""#));
    }

    #[test]
    fn token_entry_skips_label_when_absent() {
        let body = serde_json::to_string(&TokenEntry {
            project: "demo".to_string(),
            masked: "nt_push_…abcd".to_string(),
            added_at: "2026-05-12T00:00:00Z".to_string(),
            label: None,
        })
        .unwrap();
        assert!(!body.contains(r#""label""#), "got: {body}");
    }

    #[test]
    fn token_entry_includes_label_when_present() {
        let body = serde_json::to_string(&TokenEntry {
            project: "demo".to_string(),
            masked: "nt_push_…abcd".to_string(),
            added_at: "2026-05-12T00:00:00Z".to_string(),
            label: Some("dev".to_string()),
        })
        .unwrap();
        assert!(body.contains(r#""label":"dev""#));
    }
}
