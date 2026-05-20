//! Auth-resolution tests for `nt publish` — the push-token-from-
//! config.json path (this fix) and the env-var escape hatch.
//!
//! Architectural rule pinned here: **publish must only ever send the
//! push token registered for `--project`** (or `NO_TICKETS_TOKEN` if
//! set as a CI escape hatch). Session credentials from `nt init` are
//! a management-API identity and MUST NOT reach `/v1/events`. See
//! `docs/fixes/publish-uses-push-token.md`.
//!
//! Tests intentionally cover both directions:
//! - the new behaviour (push token from config.json, env-var wins)
//! - the architectural pin (session creds never consulted, even when
//!   present on disk)
//!
//! The session-never-consulted pin is the load-bearing one — without
//! it, a regression that reintroduces the `resolve_auth` fallback would
//! pass every "happy path" test and silently leak session creds to the
//! publish endpoint again.
//!
//! Path scheme: `$NO_TICKETS_HOME/.notickets/{config.json,credentials}`,
//! matching the harness's `tempdir` convention. The Rust binary reads
//! the same files regardless of platform under the test override.

use std::fs;

use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::{base_args_with_data, run_nt_publish, tempdir};

const PUSH_TOKEN_REGISTERED: &str = "nt_push_registered_for_demo_project";
const PUSH_TOKEN_ENV_OVERRIDE: &str = "nt_push_env_var_escape_hatch";
const SESSION_TOKEN_NEVER_SEND: &str = "nt_session_must_never_reach_events_endpoint";

/// Helper: write a config.json with one project entry under
/// `$home/.notickets/config.json`, mirroring the format `token add`
/// produces and `paths::config_dir` reads (with NO_TICKETS_HOME
/// override pointing at `$home/.notickets`).
fn write_config_with_project(home: &std::path::Path, project: &str, push_token: &str) {
    let dir = home.join(".notickets");
    fs::create_dir_all(&dir).expect("create .notickets dir");
    let cfg = json!({
        "projects": {
            project: {
                "pushToken": push_token,
                "addedAt": "2026-05-20T00:00:00.000Z",
            }
        }
    });
    fs::write(
        dir.join("config.json"),
        serde_json::to_string_pretty(&cfg).unwrap(),
    )
    .expect("write config.json");
}

/// Helper: write a `credentials` file that the OLD code path would
/// load + send. Used by the architectural-pin tests below to assert
/// publish does NOT consult this file.
fn write_session_credentials(home: &std::path::Path, host: &str, token: &str) {
    let dir = home.join(".notickets");
    fs::create_dir_all(&dir).expect("create .notickets dir");
    let creds = json!({
        "token": token,
        "email": "test@example.com",
        "expiresAt": "2099-01-01T00:00:00.000Z",
        "host": host,
    });
    fs::write(
        dir.join("credentials"),
        serde_json::to_string(&creds).unwrap(),
    )
    .expect("write credentials");
}

// ─── A. config.json push token is sent as Bearer when --project resolves ───

#[tokio::test]
async fn publish_uses_push_token_from_config_json_when_no_env_token() {
    let server = MockServer::start().await;
    let captured_auth: std::sync::Arc<std::sync::Mutex<Option<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let captured_for_responder = captured_auth.clone();

    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(move |req: &wiremock::Request| {
            let auth_header = req
                .headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            *captured_for_responder.lock().unwrap() = auth_header;
            ResponseTemplate::new(200).set_body_raw(
                r#"{"ingested":1,"deduped":0,"ids":["x"]}"#,
                "application/json",
            )
        })
        .expect(1)
        .mount(&server)
        .await;

    let home = tempdir();
    write_config_with_project(home.path(), "demo", PUSH_TOKEN_REGISTERED);

    let out = run_nt_publish(
        &server.uri(),
        None, // no NO_TICKETS_TOKEN env var
        home.path(),
        &base_args_with_data("{}"),
    )
    .await;

    assert_eq!(
        out.code, 0,
        "publish must succeed using the config.json push token; stderr={:?}",
        out.stderr
    );
    let auth = captured_auth.lock().unwrap().clone();
    assert_eq!(
        auth.as_deref(),
        Some(format!("Bearer {}", PUSH_TOKEN_REGISTERED).as_str()),
        "Bearer header must carry the push token registered in config.json for --project demo",
    );
}

// ─── B. NO_TICKETS_TOKEN env wins over config.json ────────────────────────

#[tokio::test]
async fn no_tickets_token_env_var_wins_over_config_json_push_token() {
    let server = MockServer::start().await;
    let captured_auth: std::sync::Arc<std::sync::Mutex<Option<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let captured_for_responder = captured_auth.clone();

    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(move |req: &wiremock::Request| {
            let auth_header = req
                .headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            *captured_for_responder.lock().unwrap() = auth_header;
            ResponseTemplate::new(200).set_body_raw(
                r#"{"ingested":1,"deduped":0,"ids":["x"]}"#,
                "application/json",
            )
        })
        .expect(1)
        .mount(&server)
        .await;

    let home = tempdir();
    // Register a token in config.json that we expect NOT to be used,
    // because the env var should override.
    write_config_with_project(home.path(), "demo", PUSH_TOKEN_REGISTERED);

    let out = run_nt_publish(
        &server.uri(),
        Some(PUSH_TOKEN_ENV_OVERRIDE),
        home.path(),
        &base_args_with_data("{}"),
    )
    .await;

    assert_eq!(out.code, 0, "publish must succeed; stderr={:?}", out.stderr);
    let auth = captured_auth.lock().unwrap().clone();
    assert_eq!(
        auth.as_deref(),
        Some(format!("Bearer {}", PUSH_TOKEN_ENV_OVERRIDE).as_str()),
        "NO_TICKETS_TOKEN env var must override the config.json registered token",
    );
}

// ─── C. Missing project in config.json → ProjectNotRegistered (exit 6) ────

#[tokio::test]
async fn publish_with_unregistered_project_exits_six_and_does_not_hit_server() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0) // must NOT be hit — we should fail before transport
        .mount(&server)
        .await;

    let home = tempdir();
    // No config.json at all; no env token; --project demo → unregistered.

    let out = run_nt_publish(&server.uri(), None, home.path(), &base_args_with_data("{}")).await;

    assert_eq!(
        out.code, 6,
        "unregistered --project must produce project_not_registered (exit 6); stderr={:?}",
        out.stderr
    );
    assert!(
        out.stderr.contains("\"project_not_registered\""),
        "stderr must carry the structured class; got: {:?}",
        out.stderr,
    );
    assert!(
        out.stderr.contains("demo"),
        "stderr must name the offending --project; got: {:?}",
        out.stderr,
    );
}

#[tokio::test]
async fn project_not_registered_payload_lists_known_projects_when_some_are_registered() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let home = tempdir();
    // Register a couple of other projects but NOT "demo".
    write_config_with_project(home.path(), "other-staging", "nt_push_other_a");
    // Append a second project to the same config (append-style write).
    let dir = home.path().join(".notickets");
    let cfg = json!({
        "projects": {
            "other-staging": { "pushToken": "nt_push_other_a", "addedAt": "2026-05-20T00:00:00.000Z" },
            "another-staging": { "pushToken": "nt_push_other_b", "addedAt": "2026-05-20T00:00:00.000Z" },
        }
    });
    fs::write(
        dir.join("config.json"),
        serde_json::to_string_pretty(&cfg).unwrap(),
    )
    .unwrap();

    let out = run_nt_publish(&server.uri(), None, home.path(), &base_args_with_data("{}")).await;

    assert_eq!(out.code, 6, "exit must be 6; stderr={:?}", out.stderr);
    // Find the JSON error line on stderr and parse it.
    let line = out
        .stderr
        .lines()
        .find(|l| l.contains("\"project_not_registered\""))
        .expect("structured error line on stderr");
    let parsed: Value = serde_json::from_str(line).expect("structured error line is valid JSON");
    assert_eq!(parsed["error"], "project_not_registered");
    assert_eq!(parsed["project"], "demo");
    let known = parsed["knownProjects"]
        .as_array()
        .expect("knownProjects is an array");
    let names: Vec<&str> = known.iter().filter_map(|v| v.as_str()).collect();
    assert!(
        names.contains(&"other-staging") && names.contains(&"another-staging"),
        "knownProjects must list registered names; got: {names:?}",
    );
}

// ─── D. Session creds + no push token → ProjectNotRegistered, NOT leak ────

#[tokio::test]
async fn publish_does_not_fall_back_to_session_credentials_when_project_unregistered() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0) // must NOT be hit — session must NOT leak to /v1/events
        .mount(&server)
        .await;

    let home = tempdir();
    // The "trap": a perfectly valid session credentials file on disk,
    // matching the same host the publish will resolve to. The OLD
    // resolve_auth behaviour would happily load this and send the
    // session token as Bearer. The new behaviour must IGNORE it.
    write_session_credentials(home.path(), &server.uri(), SESSION_TOKEN_NEVER_SEND);
    // No config.json — so --project demo is unregistered.

    let out = run_nt_publish(&server.uri(), None, home.path(), &base_args_with_data("{}")).await;

    assert_eq!(
        out.code, 6,
        "must be project_not_registered (exit 6), NOT not_authenticated (5) — and certainly not a successful publish; stderr={:?}",
        out.stderr,
    );
    assert!(
        !out.stderr.contains(SESSION_TOKEN_NEVER_SEND),
        "session token MUST NOT leak into the error payload; got: {:?}",
        out.stderr,
    );
}

// ─── E. Session creds + registered push token → push token wins ──────────

#[tokio::test]
async fn push_token_from_config_wins_when_session_credentials_also_present() {
    let server = MockServer::start().await;
    let captured_auth: std::sync::Arc<std::sync::Mutex<Option<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let captured_for_responder = captured_auth.clone();

    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(move |req: &wiremock::Request| {
            let auth_header = req
                .headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            *captured_for_responder.lock().unwrap() = auth_header;
            ResponseTemplate::new(200).set_body_raw(
                r#"{"ingested":1,"deduped":0,"ids":["x"]}"#,
                "application/json",
            )
        })
        .expect(1)
        .mount(&server)
        .await;

    let home = tempdir();
    // Both files present — the load-bearing combination.
    write_session_credentials(home.path(), &server.uri(), SESSION_TOKEN_NEVER_SEND);
    write_config_with_project(home.path(), "demo", PUSH_TOKEN_REGISTERED);

    let out = run_nt_publish(&server.uri(), None, home.path(), &base_args_with_data("{}")).await;

    assert_eq!(out.code, 0, "publish must succeed; stderr={:?}", out.stderr);
    let auth = captured_auth.lock().unwrap().clone();
    assert_eq!(
        auth.as_deref(),
        Some(format!("Bearer {}", PUSH_TOKEN_REGISTERED).as_str()),
        "with both creds + push token present, the push token must win",
    );
}
