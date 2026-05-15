//! Task 15 + Task 18: optional metadata flags + machine-hash attribute
//! on the wire body. Each test mounts a wiremock that records the
//! request body so assertions can pin both the *presence* and
//! *placement* of each optional field. Field-shape parity with the TS
//! reference (`src/cli/commands/publish/single.ts`) is the contract: a
//! field is OMITTED when the flag is absent (no JSON null, no empty
//! string), and the on-wire order is `type, data, subject?, source,
//! parentEventId?, traceId?, dedupeKey?`.

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::{
    base_args, capture_publish_body, envelope, run_nt_publish, run_nt_publish_with_env, tempdir,
};

#[tokio::test]
async fn publish_emits_subject_when_both_subject_flags_are_set() {
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;

    let home = tempdir();
    let mut args = base_args();
    args.extend(["--subject-type", "task", "--subject-id", "task-42"]);
    let out = run_nt_publish(&server.uri(), Some("nt_push_token"), home.path(), &args).await;
    assert_eq!(out.code, 0, "unexpected failure: stderr={:?}", out.stderr);

    let body = captured.lock().unwrap().clone().expect("captured body");
    let env = envelope(&body);
    assert_eq!(env["subject"]["type"], "task");
    assert_eq!(env["subject"]["id"], "task-42");
}

#[tokio::test]
async fn publish_omits_subject_when_neither_flag_present() {
    // Regression pin for current spike behaviour: no subject flags →
    // the `subject` key MUST NOT appear on the wire (TS conditional-
    // spread emission; not JSON `null`, not an empty object).
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;

    let home = tempdir();
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_token"),
        home.path(),
        &base_args(),
    )
    .await;
    assert_eq!(out.code, 0, "unexpected failure: stderr={:?}", out.stderr);

    let body = captured.lock().unwrap().clone().expect("captured body");
    let env = envelope(&body);
    assert!(
        env.get("subject").is_none(),
        "subject must be omitted when no subject flags set; got {env}",
    );
}

#[tokio::test]
async fn publish_subject_type_without_subject_id_short_circuits_before_any_request() {
    // Representative end-to-end check that the usage gate runs BEFORE
    // any HTTP request leaves the binary. Symmetric subject-id-without-
    // type case, the source-attribute parse errors, and the exact error
    // message strings are covered by inline `build_metadata` tests in
    // `commands/publish.rs` — no need to pay subprocess cost for each
    // permutation here.
    //
    // Mount a mock that EXPECTS zero hits: wiremock asserts on drop, so
    // any escape past the usage gate would fail the test deterministically
    // (no port-1 flakiness, no timing window).
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/events"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;

    let home = tempdir();
    let mut args = base_args();
    args.extend(["--subject-type", "task"]);
    let out = run_nt_publish(&server.uri(), Some("nt_push_token"), home.path(), &args).await;
    // Task 26: usage errors now exit 7 (was generic exit 1). Stderr
    // still names the missing flag — the message is preserved inside
    // the structured payload.
    assert_eq!(out.code, 7, "expected usage-error exit; got {out:?}");
    assert!(
        out.stderr.contains("--subject-id"),
        "stderr must name the missing flag; got {:?}",
        out.stderr,
    );
}

#[tokio::test]
async fn publish_emits_parent_event_id_when_parent_flag_set() {
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;

    let home = tempdir();
    let mut args = base_args();
    args.extend(["--parent", "evt_parent_123"]);
    let out = run_nt_publish(&server.uri(), Some("nt_push_token"), home.path(), &args).await;
    assert_eq!(out.code, 0, "unexpected failure: stderr={:?}", out.stderr);

    let body = captured.lock().unwrap().clone().expect("captured body");
    assert_eq!(envelope(&body)["parentEventId"], "evt_parent_123");
}

#[tokio::test]
async fn publish_emits_trace_id_when_trace_flag_set() {
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;

    let home = tempdir();
    let mut args = base_args();
    args.extend(["--trace", "trace-abc"]);
    let out = run_nt_publish(&server.uri(), Some("nt_push_token"), home.path(), &args).await;
    assert_eq!(out.code, 0, "unexpected failure: stderr={:?}", out.stderr);

    let body = captured.lock().unwrap().clone().expect("captured body");
    assert_eq!(envelope(&body)["traceId"], "trace-abc");
}

#[tokio::test]
async fn publish_emits_dedupe_key_when_dedupe_key_flag_set() {
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;

    let home = tempdir();
    let mut args = base_args();
    args.extend(["--dedupe-key", "dk-001"]);
    let out = run_nt_publish(&server.uri(), Some("nt_push_token"), home.path(), &args).await;
    assert_eq!(out.code, 0, "unexpected failure: stderr={:?}", out.stderr);

    let body = captured.lock().unwrap().clone().expect("captured body");
    assert_eq!(envelope(&body)["dedupeKey"], "dk-001");
}

#[tokio::test]
async fn publish_source_name_flag_overrides_default_nt_cli() {
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;

    let home = tempdir();
    let mut args = base_args();
    args.extend(["--source-name", "my-cli-wrapper"]);
    let out = run_nt_publish(&server.uri(), Some("nt_push_token"), home.path(), &args).await;
    assert_eq!(out.code, 0, "unexpected failure: stderr={:?}", out.stderr);

    let body = captured.lock().unwrap().clone().expect("captured body");
    assert_eq!(envelope(&body)["source"]["name"], "my-cli-wrapper");
}

#[tokio::test]
async fn publish_source_attribute_flag_merges_into_attributes() {
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;

    let home = tempdir();
    let mut args = base_args();
    args.extend(["--source-attribute", "runner=github-actions"]);
    let out = run_nt_publish(&server.uri(), Some("nt_push_token"), home.path(), &args).await;
    assert_eq!(out.code, 0, "unexpected failure: stderr={:?}", out.stderr);

    let body = captured.lock().unwrap().clone().expect("captured body");
    let attrs = &envelope(&body)["source"]["attributes"];
    // Both the existing `project` AND the new flag-derived attribute
    // must appear in source.attributes.
    assert_eq!(attrs["runner"], "github-actions");
    assert_eq!(attrs["project"], "demo");
}

#[tokio::test]
async fn publish_repeated_source_attribute_last_wins_on_duplicate_key() {
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;

    let home = tempdir();
    let mut args = base_args();
    args.extend([
        "--source-attribute",
        "foo=first",
        "--source-attribute",
        "foo=second",
    ]);
    let out = run_nt_publish(&server.uri(), Some("nt_push_token"), home.path(), &args).await;
    assert_eq!(out.code, 0, "unexpected failure: stderr={:?}", out.stderr);

    let body = captured.lock().unwrap().clone().expect("captured body");
    assert_eq!(envelope(&body)["source"]["attributes"]["foo"], "second");
}

// ─── Machine-hash attribute (Task 18) ─────────────────────────────────────

/// Regression pin: without `NO_TICKETS_INCLUDE_MACHINE=1` the wire
/// body's `attributes` MUST NOT contain a `machine` key. Default-off
/// is the current behaviour; opt-in only.
#[tokio::test]
async fn publish_omits_machine_attribute_by_default() {
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;

    let home = tempdir();
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_token"),
        home.path(),
        &base_args(),
    )
    .await;
    assert_eq!(out.code, 0, "unexpected failure: stderr={:?}", out.stderr);

    let body = captured.lock().unwrap().clone().expect("captured body");
    let attrs = &envelope(&body)["source"]["attributes"];
    assert!(
        attrs.get("machine").is_none(),
        "machine attribute must be absent by default; got {attrs}",
    );
}

/// With `NO_TICKETS_INCLUDE_MACHINE=1`, the wire body carries
/// `source.attributes.machine` as a 16-char lowercase-hex string.
#[tokio::test]
async fn publish_emits_machine_attribute_when_include_machine_env_set() {
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;

    let home = tempdir();
    let out = run_nt_publish_with_env(
        &server.uri(),
        Some("nt_push_token"),
        home.path(),
        &[("NO_TICKETS_INCLUDE_MACHINE", "1")],
        &base_args(),
    )
    .await;
    assert_eq!(out.code, 0, "unexpected failure: stderr={:?}", out.stderr);

    let body = captured.lock().unwrap().clone().expect("captured body");
    let machine = envelope(&body)["source"]["attributes"]["machine"]
        .as_str()
        .map(str::to_string)
        .expect("machine attribute present");
    assert_eq!(
        machine.len(),
        16,
        "machine hash must be 16 chars; got {machine:?}"
    );
    assert!(
        machine
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "machine hash must be lowercase hex; got {machine:?}",
    );
}

/// `--source-attribute machine=manual-override` MUST win over the
/// auto-computed hash. Pinned because both paths write into the same
/// BTreeMap key; the publish builder needs the auto-hash to land
/// FIRST so the flag-provided value overwrites it (BTreeMap last-wins
/// on insert).
#[tokio::test]
async fn publish_source_attribute_machine_flag_overrides_auto_hash() {
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;

    let home = tempdir();
    let mut args = base_args();
    args.extend(["--source-attribute", "machine=manual-override"]);
    let out = run_nt_publish_with_env(
        &server.uri(),
        Some("nt_push_token"),
        home.path(),
        &[("NO_TICKETS_INCLUDE_MACHINE", "1")],
        &args,
    )
    .await;
    assert_eq!(out.code, 0, "unexpected failure: stderr={:?}", out.stderr);

    let body = captured.lock().unwrap().clone().expect("captured body");
    assert_eq!(
        envelope(&body)["source"]["attributes"]["machine"],
        "manual-override",
        "explicit --source-attribute must override the auto-computed machine hash",
    );
}

#[tokio::test]
async fn publish_optional_metadata_fields_are_omitted_when_no_flags_set() {
    // Single regression pin combining all optional fields: with none
    // of the new flags, none of the new wire keys can appear. Prevents
    // any default-emission regression that would creep in if a future
    // change defaulted `--trace` to something or always wrote
    // `dedupeKey: ""`.
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;

    let home = tempdir();
    let out = run_nt_publish(
        &server.uri(),
        Some("nt_push_token"),
        home.path(),
        &base_args(),
    )
    .await;
    assert_eq!(out.code, 0, "unexpected failure: stderr={:?}", out.stderr);

    let body = captured.lock().unwrap().clone().expect("captured body");
    for omitted in [
        r#""subject""#,
        r#""parentEventId""#,
        r#""traceId""#,
        r#""dedupeKey""#,
    ] {
        assert!(
            !body.contains(omitted),
            "{omitted} must be omitted when its flag is absent; got {body}",
        );
    }
}

#[tokio::test]
async fn publish_wire_field_order_with_all_optionals_set() {
    // ADR-2-aligned wire order: type, data, subject?, source,
    // parentEventId?, traceId?, dedupeKey?. With every optional field
    // set, the byte-position assertions cover the full envelope shape.
    let server = MockServer::start().await;
    let captured = capture_publish_body(&server).await;

    let home = tempdir();
    let mut args = base_args();
    args.extend([
        "--subject-type",
        "task",
        "--subject-id",
        "task-7",
        "--parent",
        "evt_p",
        "--trace",
        "tr",
        "--dedupe-key",
        "dk",
    ]);
    let out = run_nt_publish(&server.uri(), Some("nt_push_token"), home.path(), &args).await;
    assert_eq!(out.code, 0, "unexpected failure: stderr={:?}", out.stderr);

    let body = captured.lock().unwrap().clone().expect("captured body");
    let p = |needle: &str| {
        body.find(needle)
            .unwrap_or_else(|| panic!("missing {needle:?} in {body:?}"))
    };
    let t_type = p(r#""type":"ai.task.completed.v1""#);
    let t_data = p(r#""data":{"#);
    let t_subj = p(r#""subject":{"type":"task""#);
    let t_src = p(r#""source":{"#);
    let t_par = p(r#""parentEventId":"evt_p""#);
    let t_trc = p(r#""traceId":"tr""#);
    let t_dk = p(r#""dedupeKey":"dk""#);
    assert!(
        t_type < t_data
            && t_data < t_subj
            && t_subj < t_src
            && t_src < t_par
            && t_par < t_trc
            && t_trc < t_dk,
        "wire order must be type, data, subject, source, parentEventId, traceId, dedupeKey — got {body}",
    );
}
