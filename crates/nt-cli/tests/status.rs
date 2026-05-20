//! Integration tests for `nt status` (ADR-0002 shape).
//!
//! Output: `{ "authenticated": bool, "email"?: string, "tokens": [{project, masked, addedAt, label?}, …] }`.
//! Always exits 0 unless URL resolution fails (bad env config).

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::Path;

fn nt() -> Command {
    Command::cargo_bin("no-tickets").expect("binary built")
}

/// Clears every env var the binary reads. Sets NO_TICKETS_HOME to the
/// supplied tempdir. All status tests share this helper so per-host env
/// leakage can't influence outcomes.
fn isolate<'a>(cmd: &'a mut Command, home: &Path) -> &'a mut Command {
    cmd.env("NO_TICKETS_HOME", home)
        .env_remove("NO_TICKETS_TOKEN")
        .env_remove("NO_TICKETS_ENV")
        .env_remove("NO_TICKETS_API_URL")
        .env_remove("NO_TICKETS_AUTH_URL")
}

fn write_credentials(home: &Path, body: &str) {
    let dir = home.join(".notickets");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("credentials"), body).unwrap();
}

fn write_config(home: &Path, body: &str) {
    let dir = home.join(".notickets");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("config.json"), body).unwrap();
}

fn run_status_json(cmd: &mut Command) -> Value {
    let output = cmd.output().expect("spawned");
    assert!(output.status.success(), "expected success, got {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    serde_json::from_str(stdout.trim()).expect("valid JSON")
}

const VALID_FUTURE: &str = "2099-01-01T00:00:00.000Z";
const PROD_API: &str = "https://api.no-tickets.com";

// ─── ADR-0002 scenarios ─────────────────────────────────────────────────────

#[test]
fn status_scenario_no_session_no_tokens() {
    let temp = tempfile::tempdir().unwrap();
    let v = run_status_json(isolate(&mut nt(), temp.path()).arg("status"));
    assert_eq!(v["authenticated"], Value::Bool(false));
    assert_eq!(v["tokens"], serde_json::json!([]));
    assert!(
        v.get("email").is_none(),
        "no email when unauthenticated; got {v}"
    );
}

#[test]
fn status_scenario_no_session_but_tokens_registered() {
    let temp = tempfile::tempdir().unwrap();
    write_config(
        temp.path(),
        r#"{"projects":{"demo":{"pushToken":"nt_push_abcdefgh1111","addedAt":"2026-05-12T00:00:00Z","label":"dev"}}}"#,
    );
    let v = run_status_json(isolate(&mut nt(), temp.path()).arg("status"));
    assert_eq!(v["authenticated"], Value::Bool(false));
    assert!(v.get("email").is_none());
    let tokens = v["tokens"].as_array().unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0]["project"], "demo");
    assert_eq!(tokens[0]["masked"], "nt_push_…1111");
    assert_eq!(tokens[0]["label"], "dev");
    // Raw token MUST NOT appear in stdout.
    assert!(
        !serde_json::to_string(&v).unwrap().contains("abcdefgh"),
        "raw token leaked into status output",
    );
}

#[test]
fn status_scenario_session_plus_tokens() {
    let temp = tempfile::tempdir().unwrap();
    write_credentials(
        temp.path(),
        &format!(
            r#"{{"token":"nt_session_x","email":"alice@example.com","expiresAt":"{VALID_FUTURE}","host":"{PROD_API}"}}"#,
        ),
    );
    write_config(
        temp.path(),
        r#"{"projects":{"demo":{"pushToken":"nt_push_abcdefgh1111","addedAt":"2026-05-12T00:00:00Z"}}}"#,
    );
    let v = run_status_json(isolate(&mut nt(), temp.path()).arg("status"));
    assert_eq!(v["authenticated"], Value::Bool(true));
    assert_eq!(v["email"], "alice@example.com");
    assert_eq!(v["tokens"].as_array().unwrap().len(), 1);
}

#[test]
fn status_scenario_session_host_mismatch_emits_warning_and_authenticated_false() {
    let temp = tempfile::tempdir().unwrap();
    write_credentials(
        temp.path(),
        r#"{"token":"nt_session_staging","email":"a@b.com","expiresAt":"2099-01-01T00:00:00.000Z","host":"https://api-staging.no-tickets.com"}"#,
    );
    isolate(&mut nt(), temp.path())
        .arg("status")
        .assert()
        .success()
        // Stdout: authenticated:false, no email, empty tokens.
        .stdout(predicate::str::contains(r#""authenticated":false"#))
        .stdout(predicate::str::contains(r#""email""#).not())
        // Stderr: warning naming both hosts + `nt init` suggestion.
        .stderr(predicate::str::contains("Warning:"))
        .stderr(predicate::str::contains(
            "https://api-staging.no-tickets.com",
        ))
        .stderr(predicate::str::contains("https://api.no-tickets.com"))
        .stderr(predicate::str::contains("re-authenticate"))
        .stderr(predicate::str::contains("no-tickets init"))
        // Token MUST NOT leak.
        .stderr(predicate::str::contains("nt_session_staging").not());
}

// ─── Env-supplied tokens: NOT authenticated in status sense ────────────────

#[test]
fn status_with_no_tickets_token_env_only_is_not_authenticated() {
    // NO_TICKETS_TOKEN is a transport-level escape hatch. It doesn't
    // count as an authenticated identity — status reflects only what a
    // session (or its absence) says.
    let temp = tempfile::tempdir().unwrap();
    let v = run_status_json(
        isolate(&mut nt(), temp.path())
            .env("NO_TICKETS_TOKEN", "nt_push_envtoken1234")
            .arg("status"),
    );
    assert_eq!(v["authenticated"], Value::Bool(false));
    assert!(v.get("email").is_none());
}

// ─── Credentials file edge cases: degrade to unauthenticated, no panic ─────

#[test]
fn status_expired_credentials_count_as_unauthenticated() {
    let temp = tempfile::tempdir().unwrap();
    write_credentials(
        temp.path(),
        &format!(
            r#"{{"token":"nt_session_old","email":"a@b.com","expiresAt":"2000-01-01T00:00:00.000Z","host":"{PROD_API}"}}"#,
        ),
    );
    let v = run_status_json(isolate(&mut nt(), temp.path()).arg("status"));
    assert_eq!(v["authenticated"], Value::Bool(false));
}

#[test]
fn status_malformed_credentials_json_counts_as_unauthenticated() {
    let temp = tempfile::tempdir().unwrap();
    write_credentials(temp.path(), "{ this is not json }");
    let v = run_status_json(isolate(&mut nt(), temp.path()).arg("status"));
    assert_eq!(v["authenticated"], Value::Bool(false));
}

#[test]
fn status_credentials_missing_host_field_counts_as_unauthenticated() {
    // Legacy file (no `host`) MUST NOT silently authenticate.
    let temp = tempfile::tempdir().unwrap();
    write_credentials(
        temp.path(),
        &format!(
            r#"{{"token":"nt_session_legacy","email":"a@b.com","expiresAt":"{VALID_FUTURE}"}}"#,
        ),
    );
    let v = run_status_json(isolate(&mut nt(), temp.path()).arg("status"));
    assert_eq!(v["authenticated"], Value::Bool(false));
}

// ─── URL config errors: exit 1, stderr only (auth state irrelevant) ────────

#[test]
fn status_rejects_partial_env_url_pair_api_only() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_API_URL", "https://only-api.example")
        .arg("status")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("NO_TICKETS_API_URL"))
        .stderr(predicate::str::contains("Set both"));
}

#[test]
fn status_rejects_unknown_no_tickets_env_value() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_ENV", "qa")
        .arg("status")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("NO_TICKETS_ENV=qa"));
}

#[test]
fn status_rejects_no_tickets_env_and_explicit_pair_both_set() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_ENV", "staging")
        .env("NO_TICKETS_API_URL", "https://x-api.example")
        .env("NO_TICKETS_AUTH_URL", "https://x-auth.example")
        .arg("status")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("not both"));
}

// ─── Home resolution: NO_TICKETS_HOME beats real HOME ──────────────────────

#[test]
fn status_no_tickets_home_overrides_host_home() {
    let nt_home = tempfile::tempdir().unwrap();
    let real_home = tempfile::tempdir().unwrap();
    write_credentials(
        real_home.path(),
        &format!(
            r#"{{"token":"nt_session_host_home","email":"a@b.com","expiresAt":"{VALID_FUTURE}","host":"{PROD_API}"}}"#,
        ),
    );
    // NO_TICKETS_HOME points at an empty tempdir → no creds resolved →
    // authenticated:false even though HOME/USERPROFILE has valid creds.
    let v = run_status_json(
        nt().env("NO_TICKETS_HOME", nt_home.path())
            .env("HOME", real_home.path())
            .env("USERPROFILE", real_home.path())
            .env_remove("NO_TICKETS_TOKEN")
            .env_remove("NO_TICKETS_ENV")
            .env_remove("NO_TICKETS_API_URL")
            .env_remove("NO_TICKETS_AUTH_URL")
            .arg("status"),
    );
    assert_eq!(v["authenticated"], Value::Bool(false));
}

// ─── Broken-pipe: stdout closed by consumer exits 0, not 1 ─────────────────

#[test]
fn status_broken_pipe_on_stdout_exits_zero() {
    use std::process::{Command, Stdio};

    let temp = tempfile::tempdir().unwrap();
    let bin = assert_cmd::cargo::cargo_bin("no-tickets");
    let mut child = Command::new(&bin)
        .env("NO_TICKETS_HOME", temp.path())
        .env_remove("NO_TICKETS_TOKEN")
        .env_remove("NO_TICKETS_ENV")
        .env_remove("NO_TICKETS_API_URL")
        .env_remove("NO_TICKETS_AUTH_URL")
        .arg("status")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    drop(child.stdout.take());
    let status = child.wait().expect("wait");
    assert!(status.success(), "BrokenPipe must exit 0; got {status:?}");
}
