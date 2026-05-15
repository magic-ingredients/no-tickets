//! Integration tests for `nt validate <type> <data>` (ADR-0002 surface).
//!
//! Contract:
//! - Local schema validation against the bundled JSON Schemas. No auth,
//!   no network, no project resolution — exactly the same surface as
//!   `nt_schemas::validate(type_id, data)`.
//! - Exit codes (spike-scope subset of Task 4a's full contract):
//!   0 = valid; 1 = invalid payload / bad `--data` JSON;
//!   2 = unknown event type
//! - stdout: only `{"valid":true}` on success — must stay parsable by
//!   `| jq` consumers. Failures put their human-readable issue lines on
//!   stderr; stdout stays empty.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;

fn nt() -> Command {
    Command::cargo_bin("nt").expect("binary built")
}

/// `nt validate` is local-only. Clear every env var the binary normally
/// reads so the test can prove no auth / network / config-file path is
/// touched. NO_TICKETS_HOME points at the empty tempdir so any
/// accidental config-file lookup falls through to "absent".
fn isolate<'a>(cmd: &'a mut Command, home: &Path) -> &'a mut Command {
    cmd.env("NO_TICKETS_HOME", home)
        .env_remove("NO_TICKETS_TOKEN")
        .env_remove("NO_TICKETS_ENV")
        .env_remove("NO_TICKETS_API_URL")
        .env_remove("NO_TICKETS_AUTH_URL")
}

const VALID_TASK_DATA: &str = r#"{
  "taskId": "task-1",
  "sessionId": "session-1",
  "startedAt": "2026-05-01T00:00:00.000Z",
  "completedAt": "2026-05-01T00:00:01.000Z",
  "durationMs": 1000,
  "outcome": "success",
  "callCount": 1
}"#;

#[test]
fn validate_valid_payload_exits_zero_and_prints_valid_true() {
    // Equality on stdout — `contains` would let `{"valid":true,"x":1}`
    // slip through, but the contract is the exact one-key object.
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .args([
            "validate",
            "--type",
            "ai.task.completed.v1",
            "--data",
            VALID_TASK_DATA,
        ])
        .assert()
        .success()
        .stdout(predicate::eq("{\"valid\":true}\n"))
        .stderr(predicate::str::is_empty());
}

#[test]
fn validate_unknown_event_type_exits_two_and_names_type_on_stderr() {
    // Task 26: stderr is now structured JSON (piped → JSON shape).
    // Assert the exit code + that the type id appears in the payload;
    // exact shape pinned in tests/structured_errors/validate.rs.
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .args([
            "validate",
            "--type",
            "definitely.not.a.real.type.v999",
            "--data",
            "{}",
        ])
        .assert()
        .code(2)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("\"unknown_event_type\""))
        .stderr(predicate::str::contains("definitely.not.a.real.type.v999"));
}

#[test]
fn validate_invalid_payload_exits_one_and_lists_issues_on_stderr() {
    // Empty object is missing every required field for ai.task.completed.v1.
    // Task 26: stderr is structured JSON when piped. Parse it and pin:
    // (1) class = validation_error, (2) typeId, (3) issues array length
    // matches the schema's required-field count.
    let temp = tempfile::tempdir().unwrap();
    let output = isolate(&mut nt(), temp.path())
        .args(["validate", "--type", "ai.task.completed.v1", "--data", "{}"])
        .output()
        .expect("spawned");
    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "stdout must stay empty on failure"
    );
    let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
    let payload: serde_json::Value =
        serde_json::from_str(stderr.trim_end()).expect("stderr must be a single-line JSON object");
    assert_eq!(payload["error"], "validation_error");
    assert_eq!(payload["typeId"], "ai.task.completed.v1");
    let issues = payload["issues"].as_array().expect("issues array").clone();
    assert!(
        !issues.is_empty(),
        "must report at least one issue; got: {payload:?}"
    );
    for issue in &issues {
        assert!(issue["path"].is_string(), "issue.path must be string");
        assert!(issue["message"].is_string(), "issue.message must be string");
    }
}

#[test]
fn validate_missing_required_field_surfaces_field_path_on_stderr() {
    // Drop only `taskId`; keep everything else valid. Issue list should
    // name that path so users can find the problem field.
    let mut payload: serde_json::Value =
        serde_json::from_str(VALID_TASK_DATA).expect("fixture parses");
    payload.as_object_mut().unwrap().remove("taskId");
    let data = serde_json::to_string(&payload).unwrap();

    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .args([
            "validate",
            "--type",
            "ai.task.completed.v1",
            "--data",
            &data,
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("taskId"));
}

#[test]
fn validate_bad_json_in_data_exits_seven_and_names_parse_failure() {
    // Task 26: bad input flags / values now exit 7 (usage class) instead
    // of the generic exit 1. The structured payload still names what
    // went wrong so wrappers + humans see the same context.
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .args([
            "validate",
            "--type",
            "ai.task.completed.v1",
            "--data",
            "{not valid json",
        ])
        .assert()
        .code(7)
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("\"usage\""))
        .stderr(predicate::str::contains("--data"))
        .stderr(predicate::str::contains("JSON"));
}

#[test]
fn validate_is_observably_offline() {
    // Stronger than just "no creds": point NO_TICKETS_API_URL at a
    // refused port so any accidental HTTP call would fail loudly.
    // Since `nt validate` is local-only per ADR-0002, the command must
    // still succeed. (Bind+immediate-drop yields a port that's free
    // but unconnectable for the lifetime of the test.)
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .env("NO_TICKETS_API_URL", format!("http://127.0.0.1:{port}"))
        .env("NO_TICKETS_AUTH_URL", format!("http://127.0.0.1:{port}"))
        .args([
            "validate",
            "--type",
            "ai.task.completed.v1",
            "--data",
            VALID_TASK_DATA,
        ])
        .assert()
        .success()
        .stdout(predicate::eq("{\"valid\":true}\n"));
}

#[test]
fn validate_stdout_stays_single_line_json_for_jq_pipes() {
    // Downstream `nt validate ... | jq .valid` only works if stdout is
    // a single-line JSON document with no trailing log lines.
    let temp = tempfile::tempdir().unwrap();
    let output = isolate(&mut nt(), temp.path())
        .args([
            "validate",
            "--type",
            "ai.task.completed.v1",
            "--data",
            VALID_TASK_DATA,
        ])
        .output()
        .expect("spawned");
    assert!(output.status.success(), "expected success, got {output:?}");
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let trimmed = stdout.trim_end_matches('\n');
    assert!(
        !trimmed.contains('\n'),
        "stdout must be a single line; got {stdout:?}",
    );
    let parsed: serde_json::Value =
        serde_json::from_str(trimmed).expect("stdout must be valid JSON");
    assert_eq!(parsed["valid"], serde_json::Value::Bool(true));
}
