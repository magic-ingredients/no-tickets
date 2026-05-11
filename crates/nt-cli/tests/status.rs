//! Parity tests for `nt status` against the TS reference implementation in
//! `src/sdk/auth.ts`, `src/sdk/credentials.ts`, `src/sdk/url-resolver.ts`.
//!
//! Field order on the success JSON object is fixed: authenticated, source,
//! tokenType, apiUrl, authUrl. Most tests use substring assertions to stay
//! resilient to whitespace; one canonical test parses the JSON and compares
//! semantically as the structural contract.

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use std::path::Path;

const DEFAULT_API: &str = "https://api.no-tickets.com";
const DEFAULT_AUTH: &str = "https://app.no-tickets.com/api/auth/cli";
const NOT_AUTH_MSG: &str =
    "Not authenticated. Set NO_TICKETS_TOKEN or run `npx no-tickets init`.";

fn nt() -> Command {
    Command::cargo_bin("nt").expect("binary built")
}

/// Isolate the binary from any host environment that could leak into auth /
/// URL resolution. Sets NO_TICKETS_HOME to the tempdir; clears the four env
/// vars the binary reads.
fn isolate<'a>(cmd: &'a mut Command, home: &Path) -> &'a mut Command {
    cmd.env("NO_TICKETS_HOME", home)
        .env_remove("NO_TICKETS_TOKEN")
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

fn run_status_stdout(cmd: &mut Command) -> String {
    let output = cmd.output().expect("spawned");
    assert!(output.status.success(), "expected success, got {output:?}");
    String::from_utf8(output.stdout).expect("utf8 stdout")
}

// ─── Success: structural JSON shape ─────────────────────────────────────────

#[test]
fn status_emits_structurally_correct_json_for_env_push_token() {
    let temp = tempfile::tempdir().unwrap();
    let stdout = run_status_stdout(
        isolate(&mut nt(), temp.path())
            .env("NO_TICKETS_TOKEN", "nt_push_abc123")
            .arg("status"),
    );
    let trimmed = stdout.trim_end_matches('\n');
    assert!(
        stdout.ends_with('\n'),
        "stdout must end with newline (println! / console.log parity); got {stdout:?}",
    );
    let v: Value = serde_json::from_str(trimmed).expect("valid JSON");
    assert_eq!(v["authenticated"], Value::Bool(true));
    assert_eq!(v["source"], "env");
    assert_eq!(v["tokenType"], "push");
    assert_eq!(v["apiUrl"], DEFAULT_API);
    assert_eq!(v["authUrl"], DEFAULT_AUTH);
    // No stray keys leak in.
    let obj = v.as_object().unwrap();
    let keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        vec!["authenticated", "source", "tokenType", "apiUrl", "authUrl"],
        "field order must match TS object-literal order",
    );
}

// ─── Token-type detection ───────────────────────────────────────────────────

#[test]
fn status_detects_session_token_type() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_session_xyz")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""tokenType":"session""#));
}

#[test]
fn status_detects_unknown_token_type() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "random-other-token")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""tokenType":"unknown""#));
}

// ─── Auth source: env vs credentials file ───────────────────────────────────

#[test]
fn status_falls_back_to_credentials_file_when_no_env_token() {
    let temp = tempfile::tempdir().unwrap();
    write_credentials(
        temp.path(),
        r#"{"token":"nt_push_from_file","email":"a@b.com","expiresAt":"2099-01-01T00:00:00.000Z"}"#,
    );
    isolate(&mut nt(), temp.path())
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""source":"credentials""#))
        .stdout(predicate::str::contains(r#""tokenType":"push""#));
}

#[test]
fn status_env_token_takes_precedence_over_credentials_file() {
    let temp = tempfile::tempdir().unwrap();
    write_credentials(
        temp.path(),
        r#"{"token":"nt_session_from_file","email":"a@b.com","expiresAt":"2099-01-01T00:00:00.000Z"}"#,
    );
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_env_wins")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""source":"env""#))
        .stdout(predicate::str::contains(r#""tokenType":"push""#));
}

// ─── Credentials file: invalid states all map to "not authenticated" ────────

#[test]
fn status_expired_credentials_count_as_not_authenticated() {
    let temp = tempfile::tempdir().unwrap();
    write_credentials(
        temp.path(),
        r#"{"token":"nt_push_old","email":"a@b.com","expiresAt":"2000-01-01T00:00:00.000Z"}"#,
    );
    isolate(&mut nt(), temp.path())
        .arg("status")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(NOT_AUTH_MSG));
}

/// Boundary: expiresAt exactly equal to now should NOT authenticate
/// (`new Date(parsed.expiresAt).getTime() <= Date.now()` in TS — inclusive).
/// We use the unix epoch so the comparison is unambiguously in the past.
#[test]
fn status_expiry_boundary_inclusive_past() {
    let temp = tempfile::tempdir().unwrap();
    write_credentials(
        temp.path(),
        r#"{"token":"nt_push_epoch","email":"a@b.com","expiresAt":"1970-01-01T00:00:00.000Z"}"#,
    );
    isolate(&mut nt(), temp.path())
        .arg("status")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(NOT_AUTH_MSG));
}

#[test]
fn status_malformed_credentials_json_is_not_authenticated() {
    let temp = tempfile::tempdir().unwrap();
    write_credentials(temp.path(), "{ this is not json }");
    isolate(&mut nt(), temp.path())
        .arg("status")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(NOT_AUTH_MSG));
}

#[test]
fn status_credentials_missing_required_field_is_not_authenticated() {
    let temp = tempfile::tempdir().unwrap();
    // Missing `expiresAt`. TS's isStoredCredentials shape check rejects this.
    write_credentials(
        temp.path(),
        r#"{"token":"nt_push_abc","email":"a@b.com"}"#,
    );
    isolate(&mut nt(), temp.path())
        .arg("status")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(NOT_AUTH_MSG));
}

#[test]
fn status_credentials_wrong_type_field_is_not_authenticated() {
    let temp = tempfile::tempdir().unwrap();
    // `token` is a number, not a string. TS shape check rejects this.
    write_credentials(
        temp.path(),
        r#"{"token":12345,"email":"a@b.com","expiresAt":"2099-01-01T00:00:00.000Z"}"#,
    );
    isolate(&mut nt(), temp.path())
        .arg("status")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(NOT_AUTH_MSG));
}

/// Pin Rust behaviour: an unparseable expiresAt is treated as not-authenticated.
///
/// Divergence from TS: `new Date("garbage").getTime()` returns NaN, and
/// `NaN <= now` is false in JS — TS would (accidentally) accept the
/// credential. The Rust port chooses the safer behaviour: reject. This is
/// the intended contract; the divergence is documented in
/// `docs/rust-spike-notes.md`.
#[test]
fn status_credentials_unparseable_expires_at_is_not_authenticated() {
    let temp = tempfile::tempdir().unwrap();
    write_credentials(
        temp.path(),
        r#"{"token":"nt_push_abc","email":"a@b.com","expiresAt":"not-a-date"}"#,
    );
    isolate(&mut nt(), temp.path())
        .arg("status")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(NOT_AUTH_MSG));
}

#[test]
fn status_no_env_no_file_is_not_authenticated() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .arg("status")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(NOT_AUTH_MSG));
}

// ─── Home resolution: NO_TICKETS_HOME beats real HOME ───────────────────────

/// Sets HOME to a directory containing valid credentials, and
/// NO_TICKETS_HOME to a different empty directory. The binary must read the
/// NO_TICKETS_HOME directory (which has no credentials) and report
/// not-authenticated — not pick up the HOME-side credentials.
#[test]
fn status_no_tickets_home_overrides_host_home() {
    let nt_home = tempfile::tempdir().unwrap();
    let real_home = tempfile::tempdir().unwrap();
    write_credentials(
        real_home.path(),
        r#"{"token":"nt_push_host_home","email":"a@b.com","expiresAt":"2099-01-01T00:00:00.000Z"}"#,
    );
    nt()
        .env("NO_TICKETS_HOME", nt_home.path())
        .env("HOME", real_home.path())
        .env_remove("NO_TICKETS_TOKEN")
        .env_remove("NO_TICKETS_API_URL")
        .env_remove("NO_TICKETS_AUTH_URL")
        .arg("status")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(NOT_AUTH_MSG));
}

// ─── URL resolution: defaults / env-vars / pair validation / --profile ─────

#[test]
fn status_emits_default_urls_when_no_env_no_profile() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            r#""apiUrl":"{DEFAULT_API}""#
        )))
        .stdout(predicate::str::contains(format!(
            r#""authUrl":"{DEFAULT_AUTH}""#
        )));
}

#[test]
fn status_uses_env_urls_when_both_set() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .env("NO_TICKETS_API_URL", "https://custom-api.example")
        .env("NO_TICKETS_AUTH_URL", "https://custom-auth.example")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            r#""apiUrl":"https://custom-api.example""#,
        ))
        .stdout(predicate::str::contains(
            r#""authUrl":"https://custom-auth.example""#,
        ));
}

/// Credentials file × custom URLs cross-product — exercises both axes
/// together (none of the other tests do).
#[test]
fn status_credentials_file_with_custom_env_urls() {
    let temp = tempfile::tempdir().unwrap();
    write_credentials(
        temp.path(),
        r#"{"token":"nt_session_creds","email":"a@b.com","expiresAt":"2099-01-01T00:00:00.000Z"}"#,
    );
    nt()
        .env("NO_TICKETS_HOME", temp.path())
        .env_remove("NO_TICKETS_TOKEN")
        .env("NO_TICKETS_API_URL", "https://x-api.example")
        .env("NO_TICKETS_AUTH_URL", "https://x-auth.example")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""source":"credentials""#))
        .stdout(predicate::str::contains(r#""tokenType":"session""#))
        .stdout(predicate::str::contains(
            r#""apiUrl":"https://x-api.example""#,
        ));
}

/// Whitespace-only env URL must count as unset (TS does `.trim()` then
/// length > 0), so this is NOT a partial pair — should fall through to
/// defaults, not error.
#[test]
fn status_whitespace_only_env_url_counts_as_unset() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .env("NO_TICKETS_API_URL", "   ")
        .env("NO_TICKETS_AUTH_URL", "")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            r#""apiUrl":"{DEFAULT_API}""#
        )));
}

#[test]
fn status_rejects_partial_env_url_pair_api_only() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .env("NO_TICKETS_API_URL", "https://only-api.example")
        .arg("status")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("NO_TICKETS_API_URL"))
        .stderr(predicate::str::contains("https://only-api.example"))
        .stderr(predicate::str::contains("NO_TICKETS_AUTH_URL"))
        .stderr(predicate::str::contains("Set both"));
}

#[test]
fn status_rejects_partial_env_url_pair_auth_only() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .env("NO_TICKETS_AUTH_URL", "https://only-auth.example")
        .arg("status")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("NO_TICKETS_AUTH_URL"))
        .stderr(predicate::str::contains("https://only-auth.example"))
        .stderr(predicate::str::contains("NO_TICKETS_API_URL"))
        .stderr(predicate::str::contains("Set both"));
}

#[test]
fn status_profile_flag_loads_from_config_file() {
    let temp = tempfile::tempdir().unwrap();
    write_config(
        temp.path(),
        r#"{"profiles":{"staging":{"apiUrl":"https://staging-api.example","authUrl":"https://staging-auth.example"}}}"#,
    );
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .arg("status")
        .args(["--profile", "staging"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            r#""apiUrl":"https://staging-api.example""#,
        ))
        .stdout(predicate::str::contains(
            r#""authUrl":"https://staging-auth.example""#,
        ));
}

/// clap should accept the `--profile=value` form as well as `--profile value`.
#[test]
fn status_profile_flag_accepts_equals_syntax() {
    let temp = tempfile::tempdir().unwrap();
    write_config(
        temp.path(),
        r#"{"profiles":{"staging":{"apiUrl":"https://eq-api.example","authUrl":"https://eq-auth.example"}}}"#,
    );
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .arg("status")
        .arg("--profile=staging")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            r#""apiUrl":"https://eq-api.example""#,
        ));
}

#[test]
fn status_profile_unknown_name_errors() {
    let temp = tempfile::tempdir().unwrap();
    write_config(
        temp.path(),
        r#"{"profiles":{"staging":{"apiUrl":"https://s","authUrl":"https://s"}}}"#,
    );
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .arg("status")
        .args(["--profile", "nonexistent"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("nonexistent"));
}

#[test]
fn status_profile_missing_config_file_errors() {
    let temp = tempfile::tempdir().unwrap();
    // No config.json written.
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .arg("status")
        .args(["--profile", "staging"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("staging"))
        .stderr(predicate::str::contains("config.json"));
}

#[test]
fn status_profile_config_without_profiles_key_errors() {
    let temp = tempfile::tempdir().unwrap();
    write_config(temp.path(), r#"{"something_else":true}"#);
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .arg("status")
        .args(["--profile", "staging"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("staging"));
}

#[test]
fn status_profile_config_malformed_json_errors() {
    let temp = tempfile::tempdir().unwrap();
    write_config(temp.path(), "{ not json");
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .arg("status")
        .args(["--profile", "staging"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("invalid JSON"));
}

#[test]
fn status_profile_non_http_url_errors() {
    let temp = tempfile::tempdir().unwrap();
    write_config(
        temp.path(),
        r#"{"profiles":{"staging":{"apiUrl":"ftp://nope","authUrl":"https://ok"}}}"#,
    );
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .arg("status")
        .args(["--profile", "staging"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("staging"))
        .stderr(predicate::str::contains("http"));
}
