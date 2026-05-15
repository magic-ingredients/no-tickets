//! Task 16: batch mode (`--file` / stdin). Each test pins a single
//! behaviour: happy path, stdin, empty input, malformed JSONL, unknown
//! type, per-line source merge, usage errors, missing data fallback,
//! schema-error format, empty type-id, response passthrough, missing
//! file path.

use std::process::Stdio;

use assert_cmd::cargo::cargo_bin;
use serde_json::{json, Value};
use tokio::process::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::{
    batch_file, capture_publish_body, envelope, run_nt_publish, tempdir, VALID_AI_TASK_DATA,
};

/// Single POST with a JSON array of all envelopes in declaration order.
/// `body_partial_json` matches subsequence of array elements (positional
/// match), so each envelope's `type` pins the order across the batch.
#[tokio::test]
async fn publish_batch_file_with_jsonl_sends_single_post_with_array_of_envelopes() {
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;

    let home = tempdir();
    let line1 = format!(r#"{{"type":"ai.task.completed.v1","data":{VALID_AI_TASK_DATA}}}"#);
    let line2 = format!(r#"{{"type":"ai.task.completed.v1","data":{VALID_AI_TASK_DATA}}}"#);
    let line3 = format!(r#"{{"type":"ai.task.completed.v1","data":{VALID_AI_TASK_DATA}}}"#);
    let path = batch_file(home.path(), &[line1, line2, line3]);

    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_token"),
        home.path(),
        &["--file", path.to_str().unwrap(), "--project", "demo"],
    )
    .await;
    assert_eq!(
        out.code, 0,
        "happy-path batch must succeed; stderr={:?}",
        out.stderr
    );

    let body = captured.lock().unwrap().clone().expect("captured body");
    let arr: Value = serde_json::from_str(&body).expect("body is JSON array");
    let arr = arr.as_array().expect("top-level JSON array");
    assert_eq!(arr.len(), 3, "exactly 3 envelopes on the wire; got {arr:?}");
    for envelope in arr {
        assert_eq!(envelope["type"], "ai.task.completed.v1");
        assert_eq!(envelope["source"]["name"], "nt-cli");
        assert_eq!(envelope["source"]["attributes"]["project"], "demo");
    }
}

#[tokio::test]
async fn publish_batch_stdin_dash_reads_jsonl_from_stdin() {
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;

    let home = tempdir();
    let stdin_bytes = format!(
        "{{\"type\":\"ai.task.completed.v1\",\"data\":{VALID_AI_TASK_DATA}}}\n\
         {{\"type\":\"ai.task.completed.v1\",\"data\":{VALID_AI_TASK_DATA}}}\n"
    );

    let mut cmd = Command::new(cargo_bin("nt"));
    cmd.env("NO_TICKETS_HOME", home.path())
        .env_remove("NO_TICKETS_ENV")
        .env_remove("NO_TICKETS_INCLUDE_MACHINE")
        .env("NO_TICKETS_API_URL", server.uri())
        .env("NO_TICKETS_AUTH_URL", "https://unused.example/auth")
        .env("NO_TICKETS_TOKEN", "nt_push_token")
        .env("NT_RETRY_BASE_DELAY_MS", "0")
        .args(["publish", "--file", "-", "--project", "demo"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn nt");
    {
        let mut stdin = child.stdin.take().expect("stdin pipe");
        tokio::io::AsyncWriteExt::write_all(&mut stdin, stdin_bytes.as_bytes())
            .await
            .expect("write to stdin");
        drop(stdin); // close stdin so child sees EOF
    }
    let output = child.wait_with_output().await.expect("wait child");
    let code = output.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(code, 0, "stdin batch must succeed; stderr={stderr:?}");

    let body = captured.lock().unwrap().clone().expect("captured body");
    let arr: Value = serde_json::from_str(&body).expect("body is JSON array");
    assert_eq!(
        arr.as_array().expect("array").len(),
        2,
        "stdin-fed batch must produce exactly 2 envelopes",
    );
}

#[tokio::test]
async fn publish_batch_empty_file_exits_one_and_names_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0) // empty input must short-circuit before any request
        .mount(&server)
        .await;

    let home = tempdir();
    let path = batch_file(home.path(), &[]);
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_token"),
        home.path(),
        &["--file", path.to_str().unwrap(), "--project", "demo"],
    )
    .await;
    assert_ne!(out.code, 0, "empty batch must produce non-zero exit");
    assert!(
        out.stderr.contains("empty") || out.stderr.to_lowercase().contains("no events"),
        "stderr must name the empty-batch condition; got {:?}",
        out.stderr,
    );
}

#[tokio::test]
async fn publish_batch_malformed_jsonl_reports_failing_line_number() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0) // parse failure must short-circuit
        .mount(&server)
        .await;

    let home = tempdir();
    // Line 1 valid; line 2 garbage; line 3 valid. The parser must
    // report line 2 specifically — telling the user *exactly* which
    // line failed is the contract.
    let path = batch_file(
        home.path(),
        &[
            format!(r#"{{"type":"ai.task.completed.v1","data":{VALID_AI_TASK_DATA}}}"#),
            "this is not json".to_string(),
            format!(r#"{{"type":"ai.task.completed.v1","data":{VALID_AI_TASK_DATA}}}"#),
        ],
    );
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_token"),
        home.path(),
        &["--file", path.to_str().unwrap(), "--project", "demo"],
    )
    .await;
    assert_ne!(out.code, 0, "malformed JSONL must fail");
    assert!(
        out.stderr.contains("line 2") || out.stderr.contains("line: 2"),
        "stderr must name the failing line (2); got {:?}",
        out.stderr,
    );
}

#[tokio::test]
async fn publish_batch_unknown_event_type_reports_line_number() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0) // unknown type → short-circuit before request
        .mount(&server)
        .await;

    let home = tempdir();
    let path = batch_file(
        home.path(),
        &[
            format!(r#"{{"type":"ai.task.completed.v1","data":{VALID_AI_TASK_DATA}}}"#),
            r#"{"type":"not.a.real.type.v999","data":{}}"#.to_string(),
        ],
    );
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_token"),
        home.path(),
        &["--file", path.to_str().unwrap(), "--project", "demo"],
    )
    .await;
    assert_ne!(out.code, 0);
    assert!(
        out.stderr.contains("line 2"),
        "stderr must name the line (2); got {:?}",
        out.stderr,
    );
    assert!(
        out.stderr.contains("not.a.real.type.v999")
            || out.stderr.to_lowercase().contains("unknown"),
        "stderr must name the offending type or describe it as unknown; got {:?}",
        out.stderr,
    );
}

#[tokio::test]
async fn publish_batch_per_line_source_overrides_cli_base() {
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;

    let home = tempdir();
    // Line carries its own source.name override. The CLI base is
    // "nt-cli"; per-line source.name must win on the wire.
    let path = batch_file(
        home.path(),
        &[format!(
            r#"{{"type":"ai.task.completed.v1","data":{VALID_AI_TASK_DATA},"source":{{"name":"buildkite-runner","attributes":{{"job":"42"}}}}}}"#
        )],
    );
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_token"),
        home.path(),
        &[
            "--file",
            path.to_str().unwrap(),
            "--project",
            "demo",
            "--source-attribute",
            "ci=github-actions",
        ],
    )
    .await;
    assert_eq!(
        out.code, 0,
        "per-line source override must succeed; stderr={:?}",
        out.stderr
    );

    let body = captured.lock().unwrap().clone().expect("captured body");
    let env = envelope(&body);
    // Per-line top-level field wins.
    assert_eq!(env["source"]["name"], "buildkite-runner");
    // Attributes are key-merged: CLI's `ci` survives, line's `job` is
    // present, project remains.
    let attrs = &env["source"]["attributes"];
    assert_eq!(attrs["job"], "42", "per-line attribute lands on the wire");
    assert_eq!(
        attrs["ci"], "github-actions",
        "CLI attribute survives the merge"
    );
    assert_eq!(
        attrs["project"], "demo",
        "project from CLI survives the merge"
    );
}

#[tokio::test]
async fn publish_batch_file_and_type_together_is_a_usage_error() {
    // clap's `conflicts_with = "file"` on --type produces a clap-level
    // usage error before any I/O or auth resolution happens. Pin that
    // shape so a refactor can't drift it to a runtime check.
    let home = tempdir();
    let out = run_nt_publish(
        "http://unused.example",
        Some("nt_push_token"),
        home.path(),
        &[
            "--file",
            "/dev/null",
            "--type",
            "ai.task.completed.v1",
            "--data",
            "{}",
            "--project",
            "demo",
        ],
    )
    .await;
    assert_ne!(out.code, 0, "conflicting flags must be a usage error");
    assert!(
        out.stderr.to_lowercase().contains("file")
            && (out.stderr.to_lowercase().contains("type")
                || out.stderr.to_lowercase().contains("cannot")
                || out.stderr.to_lowercase().contains("conflict")),
        "stderr must explain the conflict; got {:?}",
        out.stderr,
    );
}

#[tokio::test]
async fn publish_batch_file_with_per_event_metadata_flags_is_a_usage_error() {
    // --subject-* / --parent / --trace / --dedupe-key are single-event-
    // only: each JSONL line carries its own envelope-level metadata.
    // We reject these flags in batch mode rather than silently dropping
    // them (silent drop would be quiet data loss).
    let home = tempdir();
    let path = batch_file(
        home.path(),
        &[format!(
            r#"{{"type":"ai.task.completed.v1","data":{VALID_AI_TASK_DATA}}}"#
        )],
    );
    let out = run_nt_publish(
        "http://unused.example",
        Some("nt_push_token"),
        home.path(),
        &[
            "--file",
            path.to_str().unwrap(),
            "--project",
            "demo",
            "--parent",
            "evt_xyz",
        ],
    )
    .await;
    assert_ne!(out.code, 0, "--file + --parent must be a usage error");
    assert!(
        out.stderr.contains("--file") || out.stderr.to_lowercase().contains("batch"),
        "stderr must explain that batch mode rejects per-event flags; got {:?}",
        out.stderr,
    );
}

#[tokio::test]
async fn publish_batch_scalar_line_reports_line_number_and_object_requirement() {
    // End-to-end pin for the validate-shape branch: a scalar JSON line
    // (null, number, bool, string) parses at the JSON layer but
    // `validate_and_build_envelope` rejects it as not-an-object. Line
    // number and shape requirement land on stderr.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0) // shape rejection must short-circuit
        .mount(&server)
        .await;

    let home = tempdir();
    let valid = format!(r#"{{"type":"ai.task.completed.v1","data":{VALID_AI_TASK_DATA}}}"#);
    let path = batch_file(home.path(), &[valid, "42".to_string()]);
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_token"),
        home.path(),
        &["--file", path.to_str().unwrap(), "--project", "demo"],
    )
    .await;
    assert_ne!(
        out.code, 0,
        "scalar line must reject; stdout={:?}",
        out.stdout
    );
    assert!(
        out.stderr.contains("line 2"),
        "stderr must name the failing line; got {:?}",
        out.stderr,
    );
    assert!(
        out.stderr.to_lowercase().contains("object"),
        "stderr must explain the object-shape requirement; got {:?}",
        out.stderr,
    );
}

#[tokio::test]
async fn publish_batch_missing_data_field_falls_back_to_null_and_surfaces_schema_error() {
    // End-to-end pin for the missing-`data` → `Value::Null` fallback:
    // an entry without `data` validates against the schema as
    // `data: null`. For `ai.task.completed.v1`, that fails the schema.
    // The stderr message must name the line AND report it as a
    // validation error (not a missing-field crash or a different
    // error class).
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let home = tempdir();
    let path = batch_file(
        home.path(),
        &[r#"{"type":"ai.task.completed.v1"}"#.to_string()],
    );
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_token"),
        home.path(),
        &["--file", path.to_str().unwrap(), "--project", "demo"],
    )
    .await;
    assert_ne!(out.code, 0, "missing data must fail validation");
    assert!(
        out.stderr.contains("line 1"),
        "stderr must name the line; got {:?}",
        out.stderr,
    );
    assert!(
        out.stderr.to_lowercase().contains("validation error"),
        "stderr must surface this as a schema validation failure; got {:?}",
        out.stderr,
    );
}

#[tokio::test]
async fn publish_batch_schema_validation_failure_emits_header_plus_indented_issues() {
    // End-to-end pin for the multi-line schema-error format:
    //   "line N: K validation error(s):\n  path: message\n  path: message"
    // A future refactor that changes the indent, drops the count, or
    // re-orders the parts would land here.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let home = tempdir();
    // `taskId` and `sessionId` are required strings — passing numbers
    // makes both fail (>= 2 schema issues).
    let path = batch_file(
        home.path(),
        &[r#"{"type":"ai.task.completed.v1","data":{"taskId":42,"sessionId":99}}"#.to_string()],
    );
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_token"),
        home.path(),
        &["--file", path.to_str().unwrap(), "--project", "demo"],
    )
    .await;
    assert_ne!(out.code, 0, "schema failure must fail");
    assert!(
        out.stderr.contains("line 1"),
        "header must lead with line number; got {:?}",
        out.stderr,
    );
    assert!(
        out.stderr.contains("validation error"),
        "header must contain `validation error`; got {:?}",
        out.stderr,
    );
    // Indented per-issue line: two-space leading indent after a newline.
    assert!(
        out.stderr.contains("\n  "),
        "per-issue lines must be indented by two spaces; got {:?}",
        out.stderr,
    );
}

#[tokio::test]
async fn publish_batch_empty_type_id_is_rejected_with_line_number() {
    // Integration pin for the `!s.is_empty()` filter on the type-id at
    // the binary boundary. Without the filter, an empty type-id would
    // fall through to `nt_schemas::validate("")` which would return
    // `None` and surface as "unknown event type" — semantically the
    // same outcome but a wrong diagnostic.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let home = tempdir();
    let path = batch_file(home.path(), &[r#"{"type":"","data":{}}"#.to_string()]);
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_token"),
        home.path(),
        &["--file", path.to_str().unwrap(), "--project", "demo"],
    )
    .await;
    assert_ne!(out.code, 0);
    assert!(
        out.stderr.contains("line 1"),
        "stderr must name the line; got {:?}",
        out.stderr,
    );
    assert!(
        out.stderr.to_lowercase().contains("type"),
        "stderr must reference the type field; got {:?}",
        out.stderr,
    );
}

#[tokio::test]
async fn publish_batch_server_response_passes_through_to_stdout_verbatim() {
    // The batch path uses its own `publish_envelopes` helper (distinct
    // from single-event `publish_event`). Pin that it prints the
    // server's JSON response verbatim to stdout — including
    // forward-compat fields the binary doesn't yet know about. A
    // refactor that introduced lossy "typed-struct" parsing on the
    // batch response would silently drop unknown fields.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ingested": 2,
            "deduped": 0,
            "ids": ["evt_a", "evt_b"],
            "futureField": "preserved",
        })))
        .expect(1)
        .mount(&server)
        .await;

    let home = tempdir();
    let line = format!(r#"{{"type":"ai.task.completed.v1","data":{VALID_AI_TASK_DATA}}}"#);
    let path = batch_file(home.path(), &[line.clone(), line]);
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_token"),
        home.path(),
        &["--file", path.to_str().unwrap(), "--project", "demo"],
    )
    .await;
    assert_eq!(out.code, 0, "happy path expected; stderr={:?}", out.stderr);

    let body: Value = serde_json::from_str(out.stdout.trim()).expect("stdout is JSON");
    assert_eq!(body["ingested"], 2);
    assert_eq!(body["deduped"], 0);
    assert_eq!(body["ids"][0], "evt_a");
    assert_eq!(body["ids"][1], "evt_b");
    assert_eq!(
        body["futureField"], "preserved",
        "unknown fields on the server response must flow through stdout",
    );
}

#[tokio::test]
async fn publish_batch_missing_file_path_reports_path() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let home = tempdir();
    let nonexistent = home.path().join("does-not-exist.jsonl");
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_token"),
        home.path(),
        &["--file", nonexistent.to_str().unwrap(), "--project", "demo"],
    )
    .await;
    assert_ne!(out.code, 0, "missing file must produce non-zero exit");
    assert!(
        out.stderr.contains("does-not-exist.jsonl")
            || out.stderr.to_lowercase().contains("no such file")
            || out.stderr.to_lowercase().contains("could not read"),
        "stderr must surface the missing-file diagnostic; got {:?}",
        out.stderr,
    );
}
