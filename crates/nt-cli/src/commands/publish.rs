//! `nt publish` — single-event publish over HTTPS with Bearer auth.
//! Mirrors `src/cli/commands/publish` + `src/transport/events.ts::publish`.
//!
//! Scope: single event with bounded retry/backoff on transient failures
//! (Task 17 — retry policy + classifier live in `transport`). `--stream`
//! mode (Task 4b), batch mode (Task 16), and source auto-detection
//! (Task 18) live in their own tasks.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::auth::{emit_host_mismatch_warning, resolve_auth, AuthOutcome, NOT_AUTH_MSG};
use crate::env::Env;
use crate::transport::{
    post_json_with_retry, Client, HttpClient, RetryPolicy, Sleeper, TokioSleeper,
};
use crate::urls::resolve_urls;

pub struct PublishArgs<'a> {
    pub type_id: &'a str,
    /// Raw `--data` argument. Parsed inside `run()` so the i32 exit-code
    /// contract owns the full input-handling surface (main.rs is
    /// dispatch-only; doesn't short-circuit with its own exit calls).
    pub data: &'a str,
    pub project: &'a str,
    pub subject_type: Option<&'a str>,
    pub subject_id: Option<&'a str>,
    pub source_name: Option<&'a str>,
    /// Raw `--source-attribute KEY=VALUE` repeats. Parsed inside `run()`
    /// so usage errors flow through the same exit-1 path as the rest of
    /// the input validation.
    pub source_attributes: &'a [String],
    pub parent: Option<&'a str>,
    pub trace: Option<&'a str>,
    pub dedupe_key: Option<&'a str>,
}

/// Serialised event envelope. Field declaration order is preserved by
/// serde derive — `type, data, subject?, source, parentEventId?,
/// traceId?, dedupeKey?` — matching the TS `eventSchema` emission order.
/// Every optional field is omitted (not null, not empty string) when
/// unset, via `skip_serializing_if`. The wire-body field-order tests
/// pin this.
#[derive(Serialize)]
struct EventEnvelope<'a> {
    #[serde(rename = "type")]
    type_id: &'a str,
    data: &'a Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    subject: Option<Subject<'a>>,
    source: Source<'a>,
    #[serde(rename = "parentEventId", skip_serializing_if = "Option::is_none")]
    parent_event_id: Option<&'a str>,
    #[serde(rename = "traceId", skip_serializing_if = "Option::is_none")]
    trace_id: Option<&'a str>,
    #[serde(rename = "dedupeKey", skip_serializing_if = "Option::is_none")]
    dedupe_key: Option<&'a str>,
}

#[derive(Serialize, Debug)]
struct Subject<'a> {
    #[serde(rename = "type")]
    subject_type: &'a str,
    id: &'a str,
}

#[derive(Serialize)]
struct Source<'a> {
    name: &'a str,
    #[serde(rename = "sdkVersion")]
    sdk_version: &'a str,
    /// `attributes` is built as a single ordered map containing the
    /// `project` entry (canonical) plus any `--source-attribute`
    /// overrides. BTreeMap gives a deterministic key order — important
    /// for wire-shape stability and for `last-wins` semantics on
    /// duplicate keys (driven by insert order in `merge_attributes`).
    #[serde(skip_serializing_if = "Option::is_none")]
    attributes: Option<BTreeMap<&'a str, &'a str>>,
}

/// Stateless core: takes an injected `HttpClient`, the resolved + parsed
/// inputs, sends the publish request, maps the result to an exit code.
///
/// Production wires `Client` (reqwest); tests wire a `FakeHttpClient`
/// that records the call and returns canned responses, enabling
/// in-process coverage of the error-mapping branches without
/// subprocess-plus-wiremock. The integration tests in `tests/publish.rs`
/// still own the end-to-end transport-level coverage (real reqwest, real TLS).
///
/// Body serialisation happens here rather than at the transport
/// boundary: the wire format (single-element JSON array of envelopes)
/// is a publish-flow concern, not a transport-layer concern.
async fn publish_event<C: HttpClient, S: Sleeper>(
    client: &C,
    policy: &RetryPolicy,
    sleeper: &S,
    type_id: &str,
    data: &Value,
    meta: EventMetadata<'_>,
) -> i32 {
    let body = vec![build_envelope(type_id, data, meta)];
    // serde_json::to_vec on `Vec<EventEnvelope>` cannot fail — every
    // field is a primitive Serialize impl over owned/borrowed data —
    // so .expect is appropriate here. A panic would indicate a bug
    // in serde, not a runtime condition.
    let body_bytes = serde_json::to_vec(&body).expect("envelope vec always serialises");
    match post_json_with_retry(client, policy, sleeper, "/v1/events", &body_bytes).await {
        Ok(response) => {
            // Server response shape: `{ ingested, deduped, ids }`.
            // serde_json::Value serialisation cannot fail for valid
            // Value, so `.expect` is appropriate here.
            println!(
                "{}",
                serde_json::to_string(&response).expect("serde_json::Value always serialises"),
            );
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

/// Already-validated metadata for a single event. Construct via
/// `build_metadata`, which enforces the subject pair invariant and
/// pre-merges `--source-attribute` flags into `attributes` (project
/// entry + per-flag overrides, last-wins on duplicate keys).
///
/// Private fields — every construction site lives in this module, so
/// the validity invariants are enforced at the boundary (no external
/// caller can hand a half-built `EventMetadata` to `build_envelope`).
/// `Debug` is required by `Result::expect_err` in the inline tests.
#[derive(Debug)]
struct EventMetadata<'a> {
    subject: Option<Subject<'a>>,
    source_name: &'a str,
    attributes: BTreeMap<&'a str, &'a str>,
    parent: Option<&'a str>,
    trace: Option<&'a str>,
    dedupe_key: Option<&'a str>,
}

/// Pure builder for a single event envelope.
///
/// Returns a single `EventEnvelope`, not a one-element Vec — the wire
/// format wraps a single event in `[...]`, but the array shape is a
/// transport concern owned by the caller (`run()` does `vec![envelope]`),
/// not by this builder.
///
/// Pure: no I/O, no env reads, no time. Field order on the wire is
/// pinned by EventEnvelope's declaration order (serde_derive emits in
/// declaration order); the inline tests assert byte-positions of every
/// envelope-level key in the serialised form.
fn build_envelope<'a>(
    type_id: &'a str,
    data: &'a Value,
    meta: EventMetadata<'a>,
) -> EventEnvelope<'a> {
    // `attributes` is always non-empty here — `build_metadata` seeds it
    // with the `project` entry on construction and every test path
    // wires through that same constructor. The Option wrapper on
    // `Source::attributes` stays so the type stays open to a future
    // "no attributes at all" branch (e.g. a `--no-source-attributes`
    // flag) without a wire-shape break.
    EventEnvelope {
        type_id,
        data,
        subject: meta.subject,
        source: Source {
            name: meta.source_name,
            sdk_version: env!("CARGO_PKG_VERSION"),
            attributes: Some(meta.attributes),
        },
        parent_event_id: meta.parent,
        trace_id: meta.trace,
        dedupe_key: meta.dedupe_key,
    }
}

pub async fn run(args: PublishArgs<'_>, env: &dyn Env) -> i32 {
    // Usage validation FIRST — before any auth/network/file-system
    // resolution — so a bad flag combo doesn't leak credentials state
    // or surface a confusing "not authenticated" message when the real
    // fault is a malformed argv.
    let meta = match build_metadata(&args) {
        Ok(m) => m,
        Err(msg) => {
            eprintln!("{msg}");
            return 1;
        }
    };

    let urls = match resolve_urls(env) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("{}", e.user_message());
            return 1;
        }
    };

    let auth = match resolve_auth(env, &urls.api_url) {
        AuthOutcome::Resolved(a) => a,
        AuthOutcome::SessionHostMismatch {
            stored_host,
            current_host,
        } => {
            emit_host_mismatch_warning(&stored_host, &current_host);
            eprintln!("{NOT_AUTH_MSG}");
            return 1;
        }
        AuthOutcome::None => {
            eprintln!("{NOT_AUTH_MSG}");
            return 1;
        }
    };

    // --data must be valid JSON. Parsing inside run() means the i32
    // exit-code contract owns the full input-handling path.
    let parsed_data: Value = match serde_json::from_str(args.data) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("--data must be valid JSON: {e}");
            return 1;
        }
    };

    let client = match Client::new(urls.api_url, auth.token) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };

    // Edge done. Delegate to the testable core.
    let policy = RetryPolicy::default_publish();
    let sleeper = TokioSleeper;
    publish_event(&client, &policy, &sleeper, args.type_id, &parsed_data, meta).await
}

/// Validate the flag combination + parse `--source-attribute` entries.
/// Returns the assembled metadata or a user-facing error string. Pure;
/// no I/O. Borrows from `args`, so the returned metadata's lifetime is
/// bounded by the caller's `args`.
fn build_metadata<'a>(args: &'a PublishArgs<'a>) -> Result<EventMetadata<'a>, String> {
    let subject = match (args.subject_type, args.subject_id) {
        (Some(t), Some(i)) => Some(Subject {
            subject_type: t,
            id: i,
        }),
        (None, None) => None,
        (Some(_), None) => return Err("--subject-type requires --subject-id".to_string()),
        (None, Some(_)) => return Err("--subject-id requires --subject-type".to_string()),
    };

    let mut attributes: BTreeMap<&'a str, &'a str> = BTreeMap::new();
    attributes.insert("project", args.project);
    for raw in args.source_attributes {
        let (key, value) = parse_source_attribute(raw)?;
        attributes.insert(key, value);
    }

    Ok(EventMetadata {
        subject,
        source_name: args.source_name.unwrap_or("nt-cli"),
        attributes,
        parent: args.parent,
        trace: args.trace,
        dedupe_key: args.dedupe_key,
    })
}

fn parse_source_attribute(raw: &str) -> Result<(&str, &str), String> {
    let Some(eq) = raw.find('=') else {
        return Err(format!(
            "--source-attribute \"{raw}\" is malformed (expected key=value)"
        ));
    };
    let key = &raw[..eq];
    if key.is_empty() {
        return Err(format!("--source-attribute \"{raw}\" has an empty key"));
    }
    let value = &raw[eq + 1..];
    Ok((key, value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::TransportError;
    use serde_json::json;
    use std::num::NonZeroU32;
    use std::sync::Mutex;
    use std::time::Duration;

    /// No-retry policy + no-op sleeper for the publish-orchestration
    /// tests below. Each test cares about a single `publish_event`
    /// branch (success / 401 / 422 / 5xx / Config); retry behaviour is
    /// owned by `transport::retry_tests` and exercised end-to-end via
    /// `tests/publish.rs`. Using `max_attempts = 1` keeps the
    /// `FakeHttpClient` response queue size at one and surfaces the
    /// recorded error verbatim, matching the original pre-Task-17 shape.
    fn no_retry() -> RetryPolicy {
        RetryPolicy {
            max_attempts: NonZeroU32::new(1).expect("1 is non-zero"),
            base_delay: Duration::from_millis(0),
            max_delay: Duration::from_millis(0),
        }
    }

    struct NoopSleeper;
    impl Sleeper for NoopSleeper {
        async fn sleep(&self, _: Duration) {
            // `max_attempts=1` means this is never invoked, but the
            // trait requires an impl.
        }
    }

    /// Records every `post_json` call and returns canned responses.
    /// One canned response per call, in FIFO order; missing canned
    /// responses panic so a test mismatch fails loudly.
    ///
    /// Async fn in trait is fine on a current-thread runtime; no Send
    /// bound issues for our usage. Mutex over the recorded calls and
    /// the response queue keeps the type Send + Sync per the trait.
    struct FakeHttpClient {
        responses: Mutex<Vec<Result<Value, TransportError>>>,
        calls: Mutex<Vec<RecordedCall>>,
    }

    #[derive(Clone, Debug)]
    struct RecordedCall {
        path: String,
        body: Vec<u8>,
    }

    impl FakeHttpClient {
        fn with_response(response: Result<Value, TransportError>) -> Self {
            Self {
                responses: Mutex::new(vec![response]),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<RecordedCall> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl HttpClient for FakeHttpClient {
        async fn post_json(&self, path: &str, body: Vec<u8>) -> Result<Value, TransportError> {
            self.calls.lock().unwrap().push(RecordedCall {
                path: path.to_string(),
                body,
            });
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                panic!(
                    "FakeHttpClient: no canned response left for call to {path:?}. \
                     Did the test set up enough .with_response() entries?",
                );
            }
            responses.remove(0)
        }
    }

    // ─── publish_event: orchestration tests via injected HttpClient ──────
    //
    // Coverage note: the success-path stdout content (`println!` of the
    // server response) is exercised end-to-end by
    // `publish_sends_post_to_v1_events_with_bearer_header_and_prints_response`
    // and `publish_response_passes_through_unknown_fields` in
    // tests/publish.rs. Capturing process stdout from inside the same
    // process requires either fd-level redirection or refactoring
    // println! through a Write-trait seam — both heavier than the
    // integration coverage that already pins the behaviour. The
    // in-process tests below cover branch selection (exit codes +
    // call shape); the integration tests own the stdout/stderr
    // contract.

    /// Construct a minimal metadata block for the publish-orchestration
    /// tests below: no subject, no flag overrides, default source.name,
    /// `project` as the only source attribute. Mirrors what
    /// `build_metadata` would produce for a `PublishArgs` with only the
    /// three required flags set.
    fn bare_meta<'a>(project: &'a str) -> EventMetadata<'a> {
        let mut attributes = BTreeMap::new();
        attributes.insert("project", project);
        EventMetadata {
            subject: None,
            source_name: "nt-cli",
            attributes,
            parent: None,
            trace: None,
            dedupe_key: None,
        }
    }

    #[tokio::test]
    async fn publish_event_returns_zero_on_2xx_response() {
        let fake = FakeHttpClient::with_response(Ok(json!({
            "ingested": 1, "deduped": 0, "ids": ["evt_1"],
        })));
        let data = json!({ "taskId": "t-1" });
        let code = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "ai.task.completed.v1",
            &data,
            bare_meta("demo"),
        )
        .await;
        assert_eq!(code, 0, "2xx must map to exit code 0");
    }

    #[tokio::test]
    async fn publish_event_posts_to_v1_events_path() {
        let fake = FakeHttpClient::with_response(Ok(json!({
            "ingested": 1, "deduped": 0, "ids": ["x"],
        })));
        let data = json!({});
        let _ = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "x.y.z.v1",
            &data,
            bare_meta("demo"),
        )
        .await;
        let calls = fake.calls();
        assert_eq!(calls.len(), 1, "exactly one HTTP call");
        assert_eq!(calls[0].path, "/v1/events", "must POST to /v1/events");
    }

    #[tokio::test]
    async fn publish_event_posts_serialised_envelope_array() {
        let fake = FakeHttpClient::with_response(Ok(json!({
            "ingested": 1, "deduped": 0, "ids": ["x"],
        })));
        let data = json!({ "taskId": "t-1" });
        let _ = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "ai.task.completed.v1",
            &data,
            bare_meta("my-project"),
        )
        .await;
        let body_bytes = fake.calls()[0].body.clone();
        let body_str = String::from_utf8(body_bytes).expect("utf8 body");
        // Single-element array wrapping the envelope
        assert!(
            body_str.starts_with('['),
            "wire body starts with [: {body_str}"
        );
        assert!(body_str.ends_with(']'), "wire body ends with ]: {body_str}");
        // Exactly one envelope — pins the "single-event" invariant so
        // a double-wrap regression (`[[{...}]]`) or accidental
        // batching can't slip through.
        assert_eq!(
            body_str.matches(r#""type":"#).count(),
            1,
            "wire body must contain exactly one envelope: {body_str}",
        );
        // Envelope fields present
        assert!(body_str.contains(r#""type":"ai.task.completed.v1""#));
        assert!(body_str.contains(r#""name":"nt-cli""#));
        assert!(body_str.contains(r#""project":"my-project""#));
    }

    #[tokio::test]
    async fn publish_event_returns_one_on_http_401_response() {
        let fake = FakeHttpClient::with_response(Err(TransportError::HttpStatus {
            status: 401,
            body: r#"{"error":"unauthorized"}"#.to_string(),
        }));
        let code = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "x.y.v1",
            &json!({}),
            bare_meta("demo"),
        )
        .await;
        assert_eq!(code, 1, "401 must map to exit code 1");
    }

    #[tokio::test]
    async fn publish_event_returns_one_on_http_422_response() {
        let fake = FakeHttpClient::with_response(Err(TransportError::HttpStatus {
            status: 422,
            body: r#"{"error":"validation_error"}"#.to_string(),
        }));
        let code = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "x.y.v1",
            &json!({}),
            bare_meta("demo"),
        )
        .await;
        assert_eq!(code, 1, "422 must map to exit code 1");
    }

    #[tokio::test]
    async fn publish_event_returns_one_on_http_5xx_response() {
        let fake = FakeHttpClient::with_response(Err(TransportError::HttpStatus {
            status: 503,
            body: String::new(),
        }));
        let code = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "x.y.v1",
            &json!({}),
            bare_meta("demo"),
        )
        .await;
        assert_eq!(code, 1, "5xx must map to exit code 1");
    }

    #[tokio::test]
    async fn publish_event_returns_one_on_config_error() {
        let fake = FakeHttpClient::with_response(Err(TransportError::Config(
            "invalid path \"/v1/events\"".to_string(),
        )));
        let code = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "x.y.v1",
            &json!({}),
            bare_meta("demo"),
        )
        .await;
        assert_eq!(code, 1, "Config error must map to exit code 1");
    }

    // ─── build_envelope: pure builder tests ──────────────────────────────

    /// Serialise a single envelope. Field-order assertions run on this
    /// string. The data payload below deliberately contains no `type`,
    /// `data`, `source`, `name`, `sdkVersion`, `attributes`, or `project`
    /// keys, so each envelope-level key appears exactly once and the
    /// assertions don't risk false positives from payload content.
    fn serialise_with_neutral_data(project: &str) -> String {
        let data = json!({ "neutralKey": "neutralValue" });
        let envelope = build_envelope("ai.task.completed.v1", &data, bare_meta(project));
        serde_json::to_string(&envelope).expect("envelope serialises")
    }

    #[test]
    fn build_envelope_field_order_is_type_data_source() {
        let body = serialise_with_neutral_data("demo");
        // Key-only locators — no coupling to value content.
        let t = body.find(r#""type":"#).expect("type key present");
        let d = body.find(r#""data":"#).expect("data key present");
        let s = body.find(r#""source":"#).expect("source key present");
        assert!(
            t < d && d < s,
            "wire field order must be type, data, source — got {body}",
        );
    }

    #[test]
    fn build_envelope_wraps_into_single_element_json_array_at_call_site() {
        // build_envelope itself returns a single EventEnvelope; the
        // array wrapping is the caller's (run's) responsibility. Pin
        // that the wire body is a JSON array containing exactly one
        // element by serialising the same wrapping run() uses.
        let data = json!({ "neutralKey": "neutralValue" });
        let body = vec![build_envelope(
            "ai.task.completed.v1",
            &data,
            bare_meta("demo"),
        )];
        let wire = serde_json::to_string(&body).expect("serialises");
        assert!(
            wire.starts_with('['),
            "wire body must start with '[': {wire}"
        );
        assert!(wire.ends_with(']'), "wire body must end with ']': {wire}");
        // Exactly one `"type":` key implies exactly one envelope.
        assert_eq!(
            wire.matches(r#""type":"#).count(),
            1,
            "wire body must contain exactly one envelope: {wire}",
        );
    }

    #[test]
    fn build_envelope_source_name_is_nt_cli() {
        let body = serialise_with_neutral_data("demo");
        assert!(
            body.contains(r#""name":"nt-cli""#),
            "source.name must be \"nt-cli\"; got {body}",
        );
    }

    #[test]
    fn build_envelope_source_sdk_version_is_crate_version() {
        let body = serialise_with_neutral_data("demo");
        let expected = format!(r#""sdkVersion":"{}""#, env!("CARGO_PKG_VERSION"));
        assert!(
            body.contains(&expected),
            "source.sdkVersion must match CARGO_PKG_VERSION; got {body}",
        );
    }

    #[test]
    fn build_envelope_writes_project_into_source_attributes() {
        let body = serialise_with_neutral_data("my-project");
        assert!(
            body.contains(r#""attributes":{"project":"my-project"}"#),
            "source.attributes.project must reflect the input; got {body}",
        );
    }

    #[test]
    fn build_envelope_emits_empty_string_when_project_is_empty() {
        // Pin behaviour for empty project — the builder does not
        // reject, validate, or substitute. Empty-string project flows
        // through verbatim. (Validation, if needed, lives outside the
        // pure builder.)
        let body = serialise_with_neutral_data("");
        assert!(
            body.contains(r#""attributes":{"project":""}"#),
            "empty project must serialise as empty string; got {body}",
        );
    }

    #[test]
    fn build_envelope_preserves_object_data_payload_verbatim() {
        let data = json!({ "taskId": "t-1", "sessionId": "s-1", "nested": { "x": 42 } });
        let envelope = build_envelope("ai.task.completed.v1", &data, bare_meta("demo"));
        let body = serde_json::to_string(&envelope).expect("serialises");
        assert!(body.contains(r#""taskId":"t-1""#));
        assert!(body.contains(r#""sessionId":"s-1""#));
        assert!(body.contains(r#""nested":{"x":42}"#));
    }

    #[test]
    fn build_envelope_preserves_string_data_payload() {
        let data = Value::String("plain-string-payload".to_string());
        let envelope = build_envelope("ai.task.completed.v1", &data, bare_meta("demo"));
        let body = serde_json::to_string(&envelope).expect("serialises");
        assert!(
            body.contains(r#""data":"plain-string-payload""#),
            "string payload must serialise as-is; got {body}",
        );
    }

    #[test]
    fn build_envelope_preserves_null_data_payload() {
        let data = Value::Null;
        let envelope = build_envelope("ai.task.completed.v1", &data, bare_meta("demo"));
        let body = serde_json::to_string(&envelope).expect("serialises");
        assert!(
            body.contains(r#""data":null"#),
            "null payload must serialise as JSON null; got {body}",
        );
    }

    #[test]
    fn build_envelope_preserves_array_data_payload() {
        let data = json!([1, 2, 3]);
        let envelope = build_envelope("ai.task.completed.v1", &data, bare_meta("demo"));
        let body = serde_json::to_string(&envelope).expect("serialises");
        assert!(
            body.contains(r#""data":[1,2,3]"#),
            "array payload must serialise as-is; got {body}",
        );
    }

    // ─── build_metadata: usage-validation tests ──────────────────────────

    fn args_with_attrs<'a>(project: &'a str, attrs: &'a [String]) -> PublishArgs<'a> {
        PublishArgs {
            type_id: "ai.task.completed.v1",
            data: "{}",
            project,
            subject_type: None,
            subject_id: None,
            source_name: None,
            source_attributes: attrs,
            parent: None,
            trace: None,
            dedupe_key: None,
        }
    }

    #[test]
    fn build_metadata_subject_type_without_id_is_usage_error() {
        let attrs: [String; 0] = [];
        let mut args = args_with_attrs("demo", &attrs);
        args.subject_type = Some("task");
        let err = build_metadata(&args).expect_err("expected usage error");
        assert!(err.contains("--subject-id"), "got {err:?}");
    }

    #[test]
    fn build_metadata_subject_id_without_type_is_usage_error() {
        let attrs: [String; 0] = [];
        let mut args = args_with_attrs("demo", &attrs);
        args.subject_id = Some("task-42");
        let err = build_metadata(&args).expect_err("expected usage error");
        assert!(err.contains("--subject-type"), "got {err:?}");
    }

    #[test]
    fn build_metadata_repeated_attribute_last_wins_on_duplicate_key() {
        let attrs = ["foo=first".to_string(), "foo=second".to_string()];
        let args = args_with_attrs("demo", &attrs);
        let meta = build_metadata(&args).expect("valid");
        assert_eq!(meta.attributes.get("foo"), Some(&"second"));
    }

    #[test]
    fn build_metadata_attribute_without_equals_is_usage_error() {
        let attrs = ["bareword".to_string()];
        let args = args_with_attrs("demo", &attrs);
        let err = build_metadata(&args).expect_err("expected usage error");
        assert!(err.contains("bareword"), "got {err:?}");
        assert!(err.contains("--source-attribute"), "got {err:?}");
    }

    #[test]
    fn build_metadata_attribute_with_empty_key_is_usage_error() {
        let attrs = ["=value".to_string()];
        let args = args_with_attrs("demo", &attrs);
        let err = build_metadata(&args).expect_err("expected usage error");
        assert!(err.contains("empty key"), "got {err:?}");
    }

    #[test]
    fn build_metadata_attribute_with_empty_value_is_accepted() {
        // TS parity (src/cli/lib/source-flags.ts): empty value is fine,
        // empty key is the only thing rejected. Pin behaviour so a
        // future "strict mode" doesn't silently drift away from the
        // wrapper-shared contract.
        let attrs = ["foo=".to_string()];
        let args = args_with_attrs("demo", &attrs);
        let meta = build_metadata(&args).expect("empty value must be accepted");
        assert_eq!(meta.attributes.get("foo"), Some(&""));
    }
}
