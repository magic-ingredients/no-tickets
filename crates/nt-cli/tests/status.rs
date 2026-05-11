use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

const DEFAULT_API: &str = "https://api.no-tickets.com";
const DEFAULT_AUTH: &str = "https://app.no-tickets.com/api/auth/cli";
const NOT_AUTH_MSG: &str =
    "Not authenticated. Set NO_TICKETS_TOKEN or run `npx no-tickets init`.";

fn nt() -> Command {
    Command::cargo_bin("nt").expect("binary built")
}

fn isolate<'a>(cmd: &'a mut Command, home: &std::path::Path) -> &'a mut Command {
    cmd.env("NO_TICKETS_HOME", home)
        .env_remove("NO_TICKETS_TOKEN")
        .env_remove("NO_TICKETS_API_URL")
        .env_remove("NO_TICKETS_AUTH_URL")
}

fn write_credentials(home: &std::path::Path, body: &str) {
    let dir = home.join(".notickets");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("credentials"), body).unwrap();
}

fn write_config(home: &std::path::Path, body: &str) {
    let dir = home.join(".notickets");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("config.json"), body).unwrap();
}

#[test]
fn status_with_env_push_token_emits_expected_json() {
    let temp = tempfile::tempdir().unwrap();
    let expected = format!(
        "{}\n",
        r#"{"authenticated":true,"source":"env","tokenType":"push","apiUrl":"https://api.no-tickets.com","authUrl":"https://app.no-tickets.com/api/auth/cli"}"#
    );
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc123")
        .arg("status")
        .assert()
        .success()
        .stdout(expected);
}

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

#[test]
fn status_malformed_credentials_count_as_not_authenticated() {
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
fn status_emits_default_urls_when_no_env_no_profile() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains(&format!(
            r#""apiUrl":"{DEFAULT_API}""#
        )))
        .stdout(predicate::str::contains(&format!(
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

#[test]
fn status_rejects_partial_env_url_pair_api_only() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .env("NO_TICKETS_API_URL", "https://only-api.example")
        .arg("status")
        .assert()
        .failure()
        .stderr(predicate::str::contains("NO_TICKETS_AUTH_URL"));
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
        .stderr(predicate::str::contains("NO_TICKETS_API_URL"));
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

#[test]
fn status_unknown_profile_is_an_error() {
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
        .stderr(predicate::str::contains("nonexistent"));
}
