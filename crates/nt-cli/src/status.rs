//! `nt status` command: resolve URLs, resolve auth, print JSON to stdout.
//! Mirrors `src/cli.ts::handleStatus`.

use std::io::{self, Write};

use serde::Serialize;

use crate::auth::{NOT_AUTH_MSG, resolve_auth};
use crate::urls::resolve_urls;

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
