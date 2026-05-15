//! `nt validate` structured-error contract tests.
//!
//! Pure local validation — no auth, no network, no fixtures beyond the
//! bundled JSON Schema. Exercises three error classes documented in
//! `docs/binary-error-contract.md`:
//!
//! - `usage` (exit 7) — `--data` not valid JSON
//! - `unknown_event_type` (exit 2) — type id not in the local registry
//! - `validation_error` (exit 1) — schema validation issues

use crate::common::{run_nt, tempdir};

#[tokio::test]
async fn validate_bad_data_json_is_usage_exit_7_with_documented_shape() {
    let home = tempdir();
    let out = run_nt(
        home.path(),
        &[],
        &[
            "validate",
            "--type",
            "ai.task.completed.v1",
            "--data",
            "not json",
        ],
    )
    .await;

    assert_eq!(out.code, 7, "bad --data JSON must surface as usage exit 7");
    let v = out.stderr_json();
    assert_eq!(v["error"], "usage");
    assert!(
        v["message"].as_str().is_some_and(|m| !m.is_empty()),
        "usage variant must include a non-empty message, got: {v:?}"
    );
}

#[tokio::test]
async fn validate_unknown_event_type_is_exit_2_with_type_id_in_payload() {
    let home = tempdir();
    let out = run_nt(
        home.path(),
        &[],
        &["validate", "--type", "no.such.type.v1", "--data", "{}"],
    )
    .await;

    assert_eq!(out.code, 2, "unknown event type must surface as exit 2");
    let v = out.stderr_json();
    assert_eq!(v["error"], "unknown_event_type");
    assert_eq!(v["typeId"], "no.such.type.v1");
    assert!(
        v["suggestions"].is_array(),
        "suggestions must be an array (possibly empty), got: {v:?}"
    );
}

#[tokio::test]
async fn validate_schema_failure_is_exit_1_with_issues_list() {
    // ai.task.completed.v1 requires `taskId` and `sessionId`. Empty
    // object → 2 validation issues.
    let home = tempdir();
    let out = run_nt(
        home.path(),
        &[],
        &["validate", "--type", "ai.task.completed.v1", "--data", "{}"],
    )
    .await;

    assert_eq!(
        out.code, 1,
        "schema validation failure must surface as exit 1"
    );
    let v = out.stderr_json();
    assert_eq!(v["error"], "validation_error");
    assert_eq!(v["typeId"], "ai.task.completed.v1");
    let issues = v["issues"].as_array().expect("issues array");
    assert!(
        !issues.is_empty(),
        "validation issues must be reported, got: {v:?}"
    );
    // Each issue must carry the documented (path, message) pair.
    for issue in issues {
        assert!(
            issue["path"].is_string(),
            "issue.path must be string, got: {issue:?}"
        );
        assert!(
            issue["message"].is_string(),
            "issue.message must be string, got: {issue:?}"
        );
    }
}

#[tokio::test]
async fn validate_success_exits_zero_with_no_stderr() {
    let home = tempdir();
    let out = run_nt(
        home.path(),
        &[],
        &[
            "validate",
            "--type",
            "ai.task.completed.v1",
            "--data",
            r#"{"taskId":"t-1","sessionId":"s-1"}"#,
        ],
    )
    .await;

    assert_eq!(
        out.code, 0,
        "valid payload must exit 0, got stderr: {}",
        out.stderr
    );
    assert!(
        out.stderr.is_empty(),
        "success path must not write to stderr, got: {}",
        out.stderr
    );
}
