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
const NOT_AUTH_MSG: &str = "Not authenticated. Set NO_TICKETS_TOKEN or run `npx no-tickets init`.";

fn nt() -> Command {
    Command::cargo_bin("nt").expect("binary built")
}

/// Isolate the binary from any host environment that could leak into auth /
/// URL resolution. Sets NO_TICKETS_HOME to the tempdir; clears the env vars
/// the binary reads.
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
    assert!(
        stdout.ends_with('\n'),
        "stdout must end with newline (println! / console.log parity); got {stdout:?}",
    );
    let trimmed = stdout.trim_end_matches('\n');

    // Semantic check: parse as JSON, assert all five fields are present and
    // exactly correct, no stray keys. (Uses serde_json's default Map, which
    // is not insertion-ordered — so we don't rely on its key order here.)
    let v: Value = serde_json::from_str(trimmed).expect("valid JSON");
    assert_eq!(v["authenticated"], Value::Bool(true));
    assert_eq!(v["source"], "env");
    assert_eq!(v["tokenType"], "push");
    assert_eq!(v["apiUrl"], DEFAULT_API);
    assert_eq!(v["authUrl"], DEFAULT_AUTH);
    assert_eq!(
        v.as_object().unwrap().len(),
        5,
        "no stray keys allowed on the status payload",
    );

    // Wire-format check: pin field order by monotonic byte positions in the
    // raw output. Decouples the contract from serde_json's internal Map
    // ordering — we assert what crosses the stdout boundary, not how it was
    // built. Matches the TS object-literal emission order.
    let p = |needle: &str| {
        trimmed
            .find(needle)
            .unwrap_or_else(|| panic!("missing {needle:?} in {trimmed:?}"))
    };
    let authenticated = p(r#""authenticated":"#);
    let source = p(r#""source":"#);
    let token_type = p(r#""tokenType":"#);
    let api_url = p(r#""apiUrl":"#);
    let auth_url = p(r#""authUrl":"#);
    assert!(
        authenticated < source && source < token_type && token_type < api_url && api_url < auth_url,
        "field order must be authenticated, source, tokenType, apiUrl, authUrl — got {trimmed}",
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
    write_credentials(temp.path(), r#"{"token":"nt_push_abc","email":"a@b.com"}"#);
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
/// Deliberate divergence from the TS reference implementation. In TS,
/// `new Date("garbage").getTime()` returns `NaN`, and `NaN <= Date.now()`
/// evaluates to `false`, so TS would (accidentally) accept the credential.
/// That's a JavaScript quirk, not a designed behaviour: a credential with
/// an unparseable expiry is not trustworthy. The Rust port rejects it.
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

// ─── Session host mismatch: emit warning, decline session (ADR-0002 Task 3) ─

/// When the credentials file's `host` field doesn't match the env-resolved
/// `api_url`, the session is stale (issued against a different env). The
/// binary must surface a warning to stderr telling the user to re-init,
/// and not silently authenticate with the stale session.
#[test]
fn status_session_host_mismatch_emits_warning_and_declines_session() {
    let temp = tempfile::tempdir().unwrap();
    write_credentials(
        temp.path(),
        r#"{"token":"nt_session_staging","email":"a@b.com","expiresAt":"2099-01-01T00:00:00.000Z","host":"https://api-staging.no-tickets.com"}"#,
    );
    isolate(&mut nt(), temp.path())
        // No env token → forces use of credentials file.
        // No NO_TICKETS_ENV → resolves to prod defaults.
        .arg("status")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("api-staging.no-tickets.com"))
        .stderr(predicate::str::contains("nt init"));
}

// ─── Home resolution: NO_TICKETS_HOME beats real HOME ───────────────────────

/// Sets host-home env vars to a directory containing valid credentials, and
/// NO_TICKETS_HOME to a different empty directory. The binary must read the
/// NO_TICKETS_HOME directory (which has no credentials) and report
/// not-authenticated — not pick up the host-home credentials.
///
/// Sets both HOME (Unix) and USERPROFILE (Windows) since the GREEN impl
/// may use a portable home-dir resolver.
#[test]
fn status_no_tickets_home_overrides_host_home() {
    let nt_home = tempfile::tempdir().unwrap();
    let real_home = tempfile::tempdir().unwrap();
    write_credentials(
        real_home.path(),
        r#"{"token":"nt_push_host_home","email":"a@b.com","expiresAt":"2099-01-01T00:00:00.000Z"}"#,
    );
    nt().env("NO_TICKETS_HOME", nt_home.path())
        .env("HOME", real_home.path())
        .env("USERPROFILE", real_home.path())
        .env_remove("NO_TICKETS_TOKEN")
        .env_remove("NO_TICKETS_API_URL")
        .env_remove("NO_TICKETS_AUTH_URL")
        .arg("status")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(NOT_AUTH_MSG));
}

// ─── URL resolution: defaults / NO_TICKETS_ENV preset / explicit pair ──────

#[test]
fn status_emits_default_urls_when_no_env_set() {
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
    nt().env("NO_TICKETS_HOME", temp.path())
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
        ))
        .stdout(predicate::str::contains(
            r#""authUrl":"https://x-auth.example""#,
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

/// Precedence: URL error wins over not-authenticated. Without this test,
/// a swapped impl (auth first, URL second) would still pass every other
/// failure-path test because each of them sets NO_TICKETS_TOKEN. Here we
/// strip the token AND leave a partial URL pair — TS reports the URL
/// error, not the auth error.
#[test]
fn status_url_error_takes_precedence_over_not_authenticated() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        // No NO_TICKETS_TOKEN, no credentials file written.
        .env("NO_TICKETS_API_URL", "https://only-api.example")
        .arg("status")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Set both"))
        .stderr(predicate::str::contains(NOT_AUTH_MSG).not());
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

// ─── URL resolution: NO_TICKETS_ENV preset (ADR-0002 layer 2) ───────────────

#[test]
fn status_emits_staging_urls_when_no_tickets_env_is_staging() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .env("NO_TICKETS_ENV", "staging")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            r#""apiUrl":"https://api-staging.no-tickets.com""#,
        ))
        .stdout(predicate::str::contains(
            r#""authUrl":"https://app-staging.no-tickets.com/api/auth/cli""#,
        ));
}

#[test]
fn status_emits_default_urls_when_no_tickets_env_is_prod() {
    // `prod` is one of three legal preset values and must produce the
    // same URLs as the unset / default case — pinned here at the
    // integration layer so the preset table's prod-row can't silently
    // drift away from the defaults.
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .env("NO_TICKETS_ENV", "prod")
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
fn status_emits_local_urls_when_no_tickets_env_is_local() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .env("NO_TICKETS_ENV", "local")
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            r#""apiUrl":"http://localhost:5002""#,
        ))
        .stdout(predicate::str::contains(
            r#""authUrl":"http://localhost:5001/api/auth/cli""#,
        ));
}

#[test]
fn status_rejects_unknown_no_tickets_env_value() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .env("NO_TICKETS_ENV", "qa")
        .arg("status")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("NO_TICKETS_ENV=qa"))
        .stderr(predicate::str::contains("staging"))
        .stderr(predicate::str::contains("local"))
        .stderr(predicate::str::contains("prod"));
}

#[test]
fn status_rejects_no_tickets_env_and_explicit_pair_both_set() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .env("NO_TICKETS_ENV", "staging")
        .env("NO_TICKETS_API_URL", "https://x-api.example")
        .env("NO_TICKETS_AUTH_URL", "https://x-auth.example")
        .arg("status")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("NO_TICKETS_ENV=staging"))
        .stderr(predicate::str::contains("NO_TICKETS_API_URL"))
        .stderr(predicate::str::contains("not both"));
}

// ─── Broken-pipe: stdout closed by consumer exits 0, not 1 ──────────────────

/// Status's stdout write must treat a `BrokenPipe` error as a normal exit
/// (consumer closed early, e.g. `nt status | head -n 0`). Any other stdout
/// error is a hard failure (exit 1). Pin both branches so mutation testing
/// has signal on the `e.kind() == io::ErrorKind::BrokenPipe` guard.
///
/// Reproduction: spawn the binary with stdout piped, drop the read end
/// before reading, wait for exit. The binary's first stdout write returns
/// `ErrorKind::BrokenPipe`; the run() handler must map that to exit code 0.
#[test]
fn status_broken_pipe_on_stdout_exits_zero() {
    use std::process::{Command, Stdio};

    let temp = tempfile::tempdir().unwrap();
    let bin = assert_cmd::cargo::cargo_bin("nt");
    let mut child = Command::new(&bin)
        .env("NO_TICKETS_HOME", temp.path())
        .env("NO_TICKETS_TOKEN", "nt_push_abc")
        .env_remove("NO_TICKETS_API_URL")
        .env_remove("NO_TICKETS_AUTH_URL")
        .arg("status")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn nt");

    // Drop the read end of stdout before the child writes — the child's
    // first println!/writeln! will fail with BrokenPipe.
    drop(child.stdout.take());

    let status = child.wait().expect("child exits");
    assert_eq!(
        status.code(),
        Some(0),
        "broken-pipe on stdout must map to exit 0, got {status:?}",
    );
}
