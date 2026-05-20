//! HTTP orchestration: serialise the envelope, post via the retry
//! transport, map the response (or transport error) to an i32 exit
//! code. Body serialisation lives here rather than at the transport
//! boundary because the wire format (single-element JSON array of
//! envelopes) is a publish-flow concern, not a transport-layer concern.

use serde_json::Value;

use crate::error::NtError;
use crate::transport::{post_json_with_retry, HttpClient, RetryPolicy, Sleeper, TransportError};

use super::envelope::{build_envelope, EventMetadata};

/// Map the transport's untyped errors onto the structured-error
/// contract. The mapping is intentionally narrow at this layer. The
/// 4xx body is preserved verbatim in the `message` field so server-
/// side validation context survives to wrappers without us having to
/// parse the server's error shape here (that's a follow-up if/when
/// the server starts returning a stable structured error body).
///
/// Status mapping:
/// - 401 → TokenRejected (publish always carries a Bearer post-fix,
///   so 401 categorically means "server rejected the token we sent",
///   not "no token to send" — that pre-flight path is
///   ProjectNotRegistered instead. Wrappers distinguish exit 8 (re-
///   mint-token-and-retry) from exit 5 (re-auth flow).)
/// - 403 → PermissionDenied (domain = "events" today; the wire layer
///   only touches `/v1/events` so we don't need to discriminate further
///   until a second domain lands)
/// - 429 → Transport { retriable: true } (rate-limited; the server is
///   asking us to back off, not refusing on merits)
/// - 5xx → Transport { retriable: true } (transient; the retry budget
///   has been exhausted by the time this maps)
/// - other 4xx → Transport { retriable: false } (terminal, caller
///   should not retry; the body is preserved in the message so server
///   validation context survives verbatim to wrappers)
pub(crate) fn map_transport_error(e: TransportError) -> NtError {
    match e {
        TransportError::Config(msg) => NtError::Usage {
            message: format!("client configuration error: {msg}"),
        },
        TransportError::Network(err) => NtError::Transport {
            message: err.to_string(),
            retriable: true,
        },
        TransportError::HttpStatus { status, body } => match status {
            401 => NtError::TokenRejected {
                message: if body.is_empty() {
                    "server rejected the bearer token (401)".to_string()
                } else {
                    format!("server rejected the bearer token (401): {body}")
                },
            },
            // 429 — rate-limited. The server is asking us to back off,
            // not refusing the request on its merits. retriable=true so
            // wrappers and batch loops back off rather than terminating.
            // (Pre-Task-26 this fell through to the generic 4xx arm and
            // got retriable=false, incorrectly classifying throttling
            // as terminal.)
            429 => NtError::Transport {
                message: if body.is_empty() {
                    "server returned 429 (rate-limited) after retries".to_string()
                } else {
                    format!("server returned 429 (rate-limited) after retries: {body}")
                },
                retriable: true,
            },
            403 => NtError::PermissionDenied {
                domain: "events".to_string(),
            },
            s if s >= 500 => NtError::Transport {
                message: if body.is_empty() {
                    format!("server returned {s} after retries")
                } else {
                    format!("server returned {s} after retries: {body}")
                },
                retriable: true,
            },
            s => NtError::Transport {
                message: if body.is_empty() {
                    format!("server returned {s}")
                } else {
                    format!("server returned {s}: {body}")
                },
                retriable: false,
            },
        },
    }
}

/// Stateless core: takes an injected `HttpClient`, the resolved + parsed
/// inputs, sends the publish request, maps the result to an exit code.
///
/// Production wires `Client` (reqwest); tests wire a `FakeHttpClient`
/// that records the call and returns canned responses, enabling
/// in-process coverage of the error-mapping branches without
/// subprocess-plus-wiremock. The integration tests in `tests/publish.rs`
/// still own the end-to-end transport-level coverage (real reqwest, real TLS).
pub(super) async fn publish_event<C: HttpClient, S: Sleeper>(
    client: &C,
    policy: &RetryPolicy,
    sleeper: &S,
    type_id: &str,
    data: &Value,
    meta: EventMetadata<'_>,
) -> Result<(), NtError> {
    let body = vec![build_envelope(type_id, data, meta)];
    // serde_json::to_vec on `Vec<EventEnvelope>` cannot fail — every
    // field is a primitive Serialize impl over owned/borrowed data —
    // so .expect is appropriate here. A panic would indicate a bug
    // in serde, not a runtime condition.
    let body_bytes = serde_json::to_vec(&body).expect("envelope vec always serialises");
    let response = post_json_with_retry(client, policy, sleeper, "/v1/events", &body_bytes)
        .await
        .map_err(map_transport_error)?;
    // Server response shape: `{ ingested, deduped, ids }`.
    println!(
        "{}",
        serde_json::to_string(&response).expect("serde_json::Value always serialises"),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::publish::envelope::bare_meta;
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

    #[tokio::test]
    async fn publish_event_returns_ok_on_2xx_response() {
        let fake = FakeHttpClient::with_response(Ok(json!({
            "ingested": 1, "deduped": 0, "ids": ["evt_1"],
        })));
        let data = json!({ "taskId": "t-1" });
        let result = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "ai.task.completed.v1",
            &data,
            bare_meta("demo"),
        )
        .await;
        assert!(result.is_ok(), "2xx must map to Ok, got: {result:?}");
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
        assert!(body_str.contains(r#""name":"no-tickets-cli""#));
        assert!(body_str.contains(r#""project":"my-project""#));
    }

    #[tokio::test]
    async fn publish_event_maps_http_401_to_token_rejected() {
        // Post the publish-uses-push-token fix, `publish` always has a
        // token to send (env-var or registered push token; missing
        // case is the pre-flight ProjectNotRegistered path). A 401
        // here means the server rejected the token we DID send. The
        // sharper class `token_rejected` (exit 8) signals that to
        // wrappers — distinct from `not_authenticated` (exit 5) which
        // is reserved for "no token to send" management-command
        // failures.
        let fake = FakeHttpClient::with_response(Err(TransportError::HttpStatus {
            status: 401,
            body: r#"{"error":"unauthorized"}"#.to_string(),
        }));
        let result = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "x.y.v1",
            &json!({}),
            bare_meta("demo"),
        )
        .await;
        assert!(
            matches!(result, Err(NtError::TokenRejected { .. })),
            "401 must map to NtError::TokenRejected, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn publish_event_maps_http_403_to_permission_denied() {
        let fake = FakeHttpClient::with_response(Err(TransportError::HttpStatus {
            status: 403,
            body: r#"{"error":"forbidden"}"#.to_string(),
        }));
        let result = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "x.y.v1",
            &json!({}),
            bare_meta("demo"),
        )
        .await;
        match result {
            Err(NtError::PermissionDenied { domain }) => {
                assert_eq!(
                    domain, "events",
                    "403 from /v1/events must carry domain=events"
                );
            }
            other => panic!("403 must map to PermissionDenied, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn publish_event_maps_http_422_to_transport_non_retriable() {
        // Body forwarded verbatim so server-side validation context
        // survives to the wrapper for surfacing to the user.
        let fake = FakeHttpClient::with_response(Err(TransportError::HttpStatus {
            status: 422,
            body: r#"{"error":"validation_error","issue":"x"}"#.to_string(),
        }));
        let result = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "x.y.v1",
            &json!({}),
            bare_meta("demo"),
        )
        .await;
        match result {
            Err(NtError::Transport { message, retriable }) => {
                assert!(!retriable, "4xx must surface as retriable=false");
                assert!(
                    message.contains("422"),
                    "message must name status: {message}"
                );
                assert!(
                    message.contains("validation_error"),
                    "server body must be preserved: {message}"
                );
            }
            other => panic!("422 must map to Transport, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn publish_event_maps_http_5xx_to_transport_retriable() {
        let fake = FakeHttpClient::with_response(Err(TransportError::HttpStatus {
            status: 503,
            body: String::new(),
        }));
        let result = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "x.y.v1",
            &json!({}),
            bare_meta("demo"),
        )
        .await;
        match result {
            Err(NtError::Transport { message, retriable }) => {
                assert!(retriable, "5xx must surface as retriable=true");
                assert!(message.contains("503"), "got: {message}");
                assert!(
                    message.contains("after retries"),
                    "5xx after retry budget must name that, got: {message}"
                );
            }
            other => panic!("5xx must map to Transport, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn publish_event_maps_config_to_usage() {
        let fake = FakeHttpClient::with_response(Err(TransportError::Config(
            "invalid path \"/v1/events\"".to_string(),
        )));
        let result = publish_event(
            &fake,
            &no_retry(),
            &NoopSleeper,
            "x.y.v1",
            &json!({}),
            bare_meta("demo"),
        )
        .await;
        assert!(
            matches!(result, Err(NtError::Usage { .. })),
            "Config must map to Usage, got: {result:?}"
        );
    }
}
