//! `nt publish` — single-event publish over HTTPS with Bearer auth.
//! Mirrors `src/cli/commands/publish` + `src/transport/events.ts::publish`.
//!
//! Spike scope (Task 14): single event only. No batching, no `--stream`
//! mode, no local schema validation, no source merging beyond
//! auto-fill, no retries. Task 5 (full CLI port) owns the rest.

use serde::Serialize;
use serde_json::Value;

use crate::auth::{NOT_AUTH_MSG, resolve_auth};
use crate::env::Env;
use crate::transport::{Client, HttpClient};
use crate::urls::resolve_urls;

pub struct PublishArgs<'a> {
    pub type_id: &'a str,
    /// Raw `--data` argument. Parsed inside `run()` so the i32 exit-code
    /// contract owns the full input-handling surface (main.rs is
    /// dispatch-only; doesn't short-circuit with its own exit calls).
    pub data: &'a str,
    pub project: &'a str,
    pub profile: Option<&'a str>,
}

/// Serialised event envelope. Field declaration order is preserved by
/// serde derive — `type` first, then `data`, then `source` — to match
/// the TS `eventSchema` emission order. The wire-body field-order test
/// pins this.
#[derive(Serialize)]
struct EventEnvelope<'a> {
    #[serde(rename = "type")]
    type_id: &'a str,
    data: &'a Value,
    source: Source<'a>,
}

#[derive(Serialize)]
struct Source<'a> {
    name: &'a str,
    #[serde(rename = "sdkVersion")]
    sdk_version: &'a str,
    /// Project name flows through `source.attributes.project` since the
    /// TS sourceSchema's `attributes: Record<string, string|number|bool>`
    /// is the documented escape hatch for caller context. The server's
    /// auth layer derives project context from the token; this field
    /// is informational for the spike.
    #[serde(skip_serializing_if = "Option::is_none")]
    attributes: Option<SourceAttributes<'a>>,
}

#[derive(Serialize)]
struct SourceAttributes<'a> {
    project: &'a str,
}

/// Stateless core: takes an injected `HttpClient`, the resolved + parsed
/// inputs, sends the publish request, maps the result to an exit code.
///
/// Production wires `Client` (reqwest); tests wire a `FakeHttpClient`
/// that records the call and returns canned responses, enabling
/// in-process coverage of the error-mapping branches without subprocess
/// + wiremock. The integration tests in `tests/publish.rs` still own
/// the end-to-end transport-level coverage (real reqwest, real TLS).
///
/// Body serialisation happens here rather than at the transport
/// boundary: the wire format (single-element JSON array of envelopes)
/// is a publish-flow concern, not a transport-layer concern.
async fn publish_event<C: HttpClient>(
    client: &C,
    type_id: &str,
    data: &Value,
    project: &str,
) -> i32 {
    let body = vec![build_envelope(type_id, data, project)];
    // serde_json::to_vec on `Vec<EventEnvelope>` cannot fail — every
    // field is a primitive Serialize impl over owned/borrowed data —
    // so .expect is appropriate here. A panic would indicate a bug
    // in serde, not a runtime condition.
    let body_bytes = serde_json::to_vec(&body)
        .expect("envelope vec always serialises");
    match client.post_json("/v1/events", body_bytes).await {
        Ok(response) => {
            // Server response shape: `{ ingested, deduped, ids }`.
            // serde_json::Value serialisation cannot fail for valid
            // Value, so `.expect` is appropriate here.
            println!(
                "{}",
                serde_json::to_string(&response)
                    .expect("serde_json::Value always serialises"),
            );
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

/// Pure builder for a single event envelope. Caller passes the resolved
/// type_id, the already-parsed data payload, and the project name.
///
/// Returns a single `EventEnvelope`, not a one-element Vec — the wire
/// format wraps a single event in `[...]`, but the array shape is a
/// transport concern owned by the caller (`run()` does `vec![envelope]`),
/// not by this builder. Keeps the builder honest about its scope
/// (single-event) and avoids advertising a batching capability that
/// doesn't exist in this spike.
///
/// Pure: no I/O, no env reads, no time. Field order on the wire is
/// pinned by EventEnvelope's declaration order (serde_derive emits in
/// declaration order) — the inline tests assert byte-positions of
/// `"type"` / `"data"` / `"source"` in the serialised form.
fn build_envelope<'a>(
    type_id: &'a str,
    data: &'a Value,
    project: &'a str,
) -> EventEnvelope<'a> {
    EventEnvelope {
        type_id,
        data,
        source: Source {
            name: "nt-cli",
            sdk_version: env!("CARGO_PKG_VERSION"),
            attributes: Some(SourceAttributes { project }),
        },
    }
}

pub async fn run(args: PublishArgs<'_>, env: &dyn Env) -> i32 {
    // URL resolution first (matches handleStatus pattern). A profile
    // error or partial-pair env-var setup wins over auth missing.
    let urls = match resolve_urls(env, args.profile) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("{}", e.user_message());
            return 1;
        }
    };

    let Some(auth) = resolve_auth(env) else {
        eprintln!("{NOT_AUTH_MSG}");
        return 1;
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
    publish_event(&client, args.type_id, &parsed_data, args.project).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::TransportError;
    use serde_json::json;
    use std::sync::Mutex;

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
        async fn post_json(
            &self,
            path: &str,
            body: Vec<u8>,
        ) -> Result<Value, TransportError> {
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

    #[tokio::test]
    async fn publish_event_returns_zero_on_2xx_response() {
        let fake = FakeHttpClient::with_response(Ok(json!({
            "ingested": 1, "deduped": 0, "ids": ["evt_1"],
        })));
        let data = json!({ "taskId": "t-1" });
        let code = publish_event(&fake, "ai.task.completed.v1", &data, "demo").await;
        assert_eq!(code, 0, "2xx must map to exit code 0");
    }

    #[tokio::test]
    async fn publish_event_posts_to_v1_events_path() {
        let fake = FakeHttpClient::with_response(Ok(json!({
            "ingested": 1, "deduped": 0, "ids": ["x"],
        })));
        let data = json!({});
        let _ = publish_event(&fake, "x.y.z.v1", &data, "demo").await;
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
        let _ = publish_event(&fake, "ai.task.completed.v1", &data, "my-project").await;
        let body_bytes = fake.calls()[0].body.clone();
        let body_str = String::from_utf8(body_bytes).expect("utf8 body");
        // Single-element array wrapping the envelope
        assert!(body_str.starts_with('['), "wire body starts with [: {body_str}");
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
        let code = publish_event(&fake, "x.y.v1", &json!({}), "demo").await;
        assert_eq!(code, 1, "401 must map to exit code 1");
    }

    #[tokio::test]
    async fn publish_event_returns_one_on_http_422_response() {
        let fake = FakeHttpClient::with_response(Err(TransportError::HttpStatus {
            status: 422,
            body: r#"{"error":"validation_error"}"#.to_string(),
        }));
        let code = publish_event(&fake, "x.y.v1", &json!({}), "demo").await;
        assert_eq!(code, 1, "422 must map to exit code 1");
    }

    #[tokio::test]
    async fn publish_event_returns_one_on_http_5xx_response() {
        let fake = FakeHttpClient::with_response(Err(TransportError::HttpStatus {
            status: 503,
            body: String::new(),
        }));
        let code = publish_event(&fake, "x.y.v1", &json!({}), "demo").await;
        assert_eq!(code, 1, "5xx must map to exit code 1");
    }

    #[tokio::test]
    async fn publish_event_returns_one_on_config_error() {
        let fake = FakeHttpClient::with_response(Err(TransportError::Config(
            "invalid path \"/v1/events\"".to_string(),
        )));
        let code = publish_event(&fake, "x.y.v1", &json!({}), "demo").await;
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
        let envelope = build_envelope("ai.task.completed.v1", &data, project);
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
        let body = vec![build_envelope("ai.task.completed.v1", &data, "demo")];
        let wire = serde_json::to_string(&body).expect("serialises");
        assert!(wire.starts_with('['), "wire body must start with '[': {wire}");
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
        let envelope = build_envelope("ai.task.completed.v1", &data, "demo");
        let body = serde_json::to_string(&envelope).expect("serialises");
        assert!(body.contains(r#""taskId":"t-1""#));
        assert!(body.contains(r#""sessionId":"s-1""#));
        assert!(body.contains(r#""nested":{"x":42}"#));
    }

    #[test]
    fn build_envelope_preserves_string_data_payload() {
        let data = Value::String("plain-string-payload".to_string());
        let envelope = build_envelope("ai.task.completed.v1", &data, "demo");
        let body = serde_json::to_string(&envelope).expect("serialises");
        assert!(
            body.contains(r#""data":"plain-string-payload""#),
            "string payload must serialise as-is; got {body}",
        );
    }

    #[test]
    fn build_envelope_preserves_null_data_payload() {
        let data = Value::Null;
        let envelope = build_envelope("ai.task.completed.v1", &data, "demo");
        let body = serde_json::to_string(&envelope).expect("serialises");
        assert!(
            body.contains(r#""data":null"#),
            "null payload must serialise as JSON null; got {body}",
        );
    }

    #[test]
    fn build_envelope_preserves_array_data_payload() {
        let data = json!([1, 2, 3]);
        let envelope = build_envelope("ai.task.completed.v1", &data, "demo");
        let body = serde_json::to_string(&envelope).expect("serialises");
        assert!(
            body.contains(r#""data":[1,2,3]"#),
            "array payload must serialise as-is; got {body}",
        );
    }
}
