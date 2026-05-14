//! Minimal HTTPS client for the publish surface. Wraps `reqwest` with
//! Bearer-auth header injection, JSON body serialisation, structured
//! error mapping, and bounded retry/backoff on transient failures
//! (Task 17). ETag / streaming remain out of scope.
//!
//! Error variants are minimal in v1 (Config / Network / HttpStatus).
//! Task 5a's 7-exit-code structured-error contract will refine this —
//! preserves enough information (full server body, reqwest error chain,
//! original config-failure message) that the future mapping is
//! straightforward and lossless.
//!
//! Retry classification (Task 17):
//! - `TransportError::Network` → transient (timeout, connect-refused, DNS)
//! - `TransportError::HttpStatus { status, .. }` with `status >= 500` → transient
//! - `TransportError::HttpStatus { status, .. }` with `status < 500` → terminal
//! - `TransportError::Config` → terminal (caller misuse, never retriable)
//!
//! POST /v1/events is retried under this policy. This diverges from the
//! TS reference (`src/transport/client.test.ts`), which only retries
//! idempotent GETs. The Rust binary makes the call retry-safe via the
//! `--dedupe-key` flag (Task 15) and server-side idempotency.

use std::fmt;
use std::time::Duration;

use serde_json::Value;

/// Transport-layer port. Production wires `Client` (reqwest-backed); tests
/// substitute a fake that records calls and returns canned responses,
/// enabling in-process coverage of `commands::publish::publish_event`'s
/// error-mapping branches without subprocess + wiremock.
///
/// Body is pre-serialised by the caller — the trait owns transport, not
/// serialisation. `Vec<u8>` flows by value so reqwest can pass it
/// straight to its request builder without an extra copy.
///
/// `Send + Sync` bounds let the trait work behind shared references in
/// async code. Even though `nt-cli`'s runtime is current-thread today,
/// future `--stream` work (Task 4b) may share a client across tasks.
pub trait HttpClient: Send + Sync {
    async fn post_json(&self, path: &str, body: Vec<u8>) -> Result<Value, TransportError>;
}

/// Default HTTP timeout. Picked generously for first-contact requests
/// against staging; can be tuned in Task 5 once we have data.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub enum TransportError {
    /// Failed to build the underlying client (TLS init, runtime
    /// configuration, etc.). Distinct from Network so callers (and
    /// future structured-error mapping) can distinguish "couldn't get
    /// off the ground" from "off the ground, but the network failed".
    Config(String),
    /// Network-level failure (DNS, TCP, TLS handshake, timeout). The
    /// full reqwest error is preserved so callers can inspect the
    /// chain (e.g., is_timeout(), is_connect()) once Task 5a's
    /// structured-error contract maps these to typed exit codes.
    Network(reqwest::Error),
    /// Server responded with a non-2xx status. Carries the status code
    /// and the raw response body so structured server-side validation
    /// messages survive verbatim to stderr.
    HttpStatus { status: u16, body: String },
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportError::Config(msg) => {
                write!(f, "client configuration error: {msg}")
            }
            TransportError::Network(e) => {
                write!(f, "transport error: {e}")
            }
            TransportError::HttpStatus { status, body } => {
                if body.is_empty() {
                    write!(f, "server returned {status}")
                } else {
                    write!(f, "server returned {status}: {body}")
                }
            }
        }
    }
}

pub struct Client {
    inner: reqwest::Client,
    base_url: url::Url,
    token: String,
}

impl Client {
    pub fn new(base_url: String, token: String) -> Result<Self, TransportError> {
        let base_url = url::Url::parse(&base_url)
            .map_err(|e| TransportError::Config(format!("invalid base URL {base_url:?}: {e}")))?;
        let inner = reqwest::Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .build()
            .map_err(|e| TransportError::Config(format!("reqwest builder: {e}")))?;
        Ok(Self {
            inner,
            base_url,
            token,
        })
    }
}

/// Production transport. Delegates to reqwest with Bearer auth header
/// injection and JSON Content-Type. Caller pre-serialises the body, so
/// the trait owns transport (URL join, header injection, status mapping)
/// without owning serialisation.
impl HttpClient for Client {
    async fn post_json(&self, path: &str, body: Vec<u8>) -> Result<Value, TransportError> {
        let url = self
            .base_url
            .join(path)
            .map_err(|e| TransportError::Config(format!("invalid path {path:?}: {e}")))?;
        let response = self
            .inner
            .post(url)
            .bearer_auth(&self.token)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(TransportError::Network)?;

        let status = response.status();
        let body_text = response.text().await.map_err(TransportError::Network)?;

        if !status.is_success() {
            return Err(TransportError::HttpStatus {
                status: status.as_u16(),
                body: body_text,
            });
        }

        if body_text.is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&body_text).map_err(|e| {
            // Server returned 2xx with a body we couldn't parse —
            // treat as a malformed response rather than a network
            // error. This is a Config-shaped problem ("we don't
            // understand the server") not a Network one.
            TransportError::Config(format!("invalid server JSON: {e}"))
        })
    }
}

// ─── Retry policy (Task 17) ──────────────────────────────────────────────

/// Bounded exponential-backoff policy for `post_json_with_retry`.
///
/// `max_attempts` counts the TOTAL attempts (1 = no retry; 3 = up to two
/// retries after the initial call). `base_delay` is the wait before the
/// 2nd attempt; subsequent waits double (`base`, `2 * base`, `4 * base`,
/// …). No jitter in v1 — added when production data justifies it.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Wired by Task 17 GREEN.
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
}

impl RetryPolicy {
    /// Production defaults: 3 attempts, 100ms base delay. Matches the TS
    /// reference's backoff schedule (100ms → 200ms between attempts) for
    /// the cases where TS does retry (idempotent GETs).
    #[allow(dead_code)] // Wired by Task 17 GREEN into commands::publish::run.
    pub fn default_publish() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
        }
    }
}

/// Abstract clock seam — production wires `TokioSleeper`; tests inject a
/// recording fake that returns immediately and captures the requested
/// durations so backoff schedule can be asserted without real waits.
#[allow(dead_code)] // Implementations are exercised by Task 17 GREEN.
pub trait Sleeper: Send + Sync {
    async fn sleep(&self, duration: Duration);
}

#[allow(dead_code)] // Constructed by Task 17 GREEN in commands::publish::run.
pub struct TokioSleeper;

impl Sleeper for TokioSleeper {
    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

/// Retries `client.post_json` on transient failures per the policy.
///
/// Returns the first successful response, or the LAST transient error if
/// every attempt failed transiently (the surfaced status / reqwest error
/// reflects the final attempt — the caller's stderr message is therefore
/// the most-recent failure mode, not the first). Terminal errors abort
/// immediately with their original variant.
///
/// Backoff: between attempt N and attempt N+1, sleeps
/// `base_delay * 2^(N-1)`. With `base_delay = 100ms` and `max_attempts =
/// 3`: 100ms before attempt 2, 200ms before attempt 3. No sleep after
/// the final attempt — give-up returns synchronously after the last
/// transport call.
#[allow(dead_code)] // Wired by Task 17 GREEN into commands::publish::run.
pub async fn post_json_with_retry<C: HttpClient, S: Sleeper>(
    _client: &C,
    _policy: &RetryPolicy,
    _sleeper: &S,
    _path: &str,
    _body: Vec<u8>,
) -> Result<Value, TransportError> {
    unimplemented!("Task 17 GREEN — bounded retry/backoff for transient publish errors")
}

/// Classifies a TransportError as retriable. Pure: no I/O, no clock.
/// Exposed for the retry loop's branching and for direct test coverage.
#[allow(dead_code)] // Exposed for retry-loop branching + direct test coverage.
pub fn is_transient(_err: &TransportError) -> bool {
    unimplemented!("Task 17 GREEN — classify Network + 5xx as transient; 4xx + Config as terminal")
}

#[cfg(test)]
mod retry_tests {
    use super::*;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    // ─── Helpers ────────────────────────────────────────────────────────

    /// Scripted `HttpClient` returning canned `post_json` results in FIFO.
    /// Records the call count so tests can assert "stopped after N
    /// attempts" without a separate counter. Missing canned responses
    /// panic to surface test/impl drift loudly.
    struct ScriptedClient {
        responses: Mutex<VecDeque<Result<Value, TransportError>>>,
        call_count: Mutex<usize>,
    }

    impl ScriptedClient {
        fn new(responses: Vec<Result<Value, TransportError>>) -> Self {
            Self {
                responses: Mutex::new(responses.into_iter().collect()),
                call_count: Mutex::new(0),
            }
        }
        fn calls(&self) -> usize {
            *self.call_count.lock().unwrap()
        }
    }

    impl HttpClient for ScriptedClient {
        async fn post_json(&self, _path: &str, _body: Vec<u8>) -> Result<Value, TransportError> {
            *self.call_count.lock().unwrap() += 1;
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("ScriptedClient ran out of canned responses — test/impl drift")
        }
    }

    /// Sleeper that records every requested duration and returns
    /// immediately. The recorded list is the backoff schedule under
    /// test — `assert_eq!(sleeper.sleeps(), vec![100ms, 200ms])` etc.
    struct RecordingSleeper {
        sleeps: Mutex<Vec<Duration>>,
    }

    impl RecordingSleeper {
        fn new() -> Self {
            Self {
                sleeps: Mutex::new(Vec::new()),
            }
        }
        fn sleeps(&self) -> Vec<Duration> {
            self.sleeps.lock().unwrap().clone()
        }
    }

    impl Sleeper for RecordingSleeper {
        async fn sleep(&self, duration: Duration) {
            self.sleeps.lock().unwrap().push(duration);
        }
    }

    fn policy(max_attempts: u32) -> RetryPolicy {
        RetryPolicy {
            max_attempts,
            // Tests assert on relative ratios via the RecordingSleeper.
            // The absolute value is irrelevant because the fake returns
            // synchronously.
            base_delay: Duration::from_millis(100),
        }
    }

    fn ok_body() -> Result<Value, TransportError> {
        Ok(json!({ "ingested": 1, "deduped": 0, "ids": ["x"] }))
    }

    fn http_503() -> Result<Value, TransportError> {
        Err(TransportError::HttpStatus {
            status: 503,
            body: String::new(),
        })
    }

    fn http_502() -> Result<Value, TransportError> {
        Err(TransportError::HttpStatus {
            status: 502,
            body: String::new(),
        })
    }

    fn http_422() -> Result<Value, TransportError> {
        Err(TransportError::HttpStatus {
            status: 422,
            body: r#"{"error":"validation_error"}"#.to_string(),
        })
    }

    fn http_401() -> Result<Value, TransportError> {
        Err(TransportError::HttpStatus {
            status: 401,
            body: String::new(),
        })
    }

    fn config_err() -> Result<Value, TransportError> {
        Err(TransportError::Config("invalid path".to_string()))
    }

    // ─── is_transient classification ────────────────────────────────────

    #[test]
    fn is_transient_classifies_5xx_as_transient() {
        let err = TransportError::HttpStatus {
            status: 503,
            body: String::new(),
        };
        assert!(is_transient(&err), "503 must be classified transient");
    }

    #[test]
    fn is_transient_classifies_500_boundary_as_transient() {
        let err = TransportError::HttpStatus {
            status: 500,
            body: String::new(),
        };
        assert!(
            is_transient(&err),
            "500 (lower 5xx boundary) must be transient",
        );
    }

    #[test]
    fn is_transient_classifies_4xx_as_terminal() {
        for code in [400u16, 401, 403, 404, 422, 499] {
            let err = TransportError::HttpStatus {
                status: code,
                body: String::new(),
            };
            assert!(
                !is_transient(&err),
                "{code} must NOT be classified transient",
            );
        }
    }

    #[test]
    fn is_transient_classifies_499_boundary_as_terminal() {
        // Pin the boundary so a `>= 499` regression can't slip through.
        let err = TransportError::HttpStatus {
            status: 499,
            body: String::new(),
        };
        assert!(
            !is_transient(&err),
            "499 (upper 4xx boundary) must be terminal",
        );
    }

    #[test]
    fn is_transient_classifies_config_as_terminal() {
        let err = TransportError::Config("invalid base URL".to_string());
        assert!(!is_transient(&err), "Config errors must be terminal");
    }

    // ─── post_json_with_retry orchestration ─────────────────────────────

    #[tokio::test]
    async fn retry_returns_success_immediately_when_first_attempt_succeeds() {
        let client = ScriptedClient::new(vec![ok_body()]);
        let sleeper = RecordingSleeper::new();
        let result =
            post_json_with_retry(&client, &policy(3), &sleeper, "/v1/events", b"[]".to_vec()).await;
        assert!(result.is_ok(), "first-attempt 2xx must return Ok");
        assert_eq!(client.calls(), 1, "no retries needed");
        assert!(
            sleeper.sleeps().is_empty(),
            "no sleep when first attempt succeeds",
        );
    }

    #[tokio::test]
    async fn retry_retries_after_503_then_returns_success() {
        let client = ScriptedClient::new(vec![http_503(), ok_body()]);
        let sleeper = RecordingSleeper::new();
        let result =
            post_json_with_retry(&client, &policy(3), &sleeper, "/v1/events", b"[]".to_vec()).await;
        assert!(
            result.is_ok(),
            "5xx → 200 must surface the 2xx success: {result:?}",
        );
        assert_eq!(client.calls(), 2, "exactly one retry observed");
        assert_eq!(sleeper.sleeps().len(), 1, "one sleep between two attempts",);
    }

    #[tokio::test]
    async fn retry_gives_up_after_max_attempts_with_last_transient_error_surfaced() {
        // All 5xx, three attempts ⇒ three calls, two sleeps, return last err.
        let client = ScriptedClient::new(vec![http_503(), http_502(), http_503()]);
        let sleeper = RecordingSleeper::new();
        let result =
            post_json_with_retry(&client, &policy(3), &sleeper, "/v1/events", b"[]".to_vec()).await;
        let err = result.expect_err("persistent 5xx must surface error");
        match err {
            TransportError::HttpStatus { status, .. } => assert_eq!(
                status, 503,
                "must surface the LAST attempt's status, not the first",
            ),
            other => panic!("expected HttpStatus error; got {other:?}"),
        }
        assert_eq!(client.calls(), 3, "exactly max_attempts calls made");
        assert_eq!(sleeper.sleeps().len(), 2, "N-1 sleeps for N attempts");
    }

    #[tokio::test]
    async fn retry_does_not_retry_on_4xx() {
        // 422 should be returned immediately even though more responses
        // are queued — the impl must not pull a second response.
        let client = ScriptedClient::new(vec![http_422()]);
        let sleeper = RecordingSleeper::new();
        let result =
            post_json_with_retry(&client, &policy(3), &sleeper, "/v1/events", b"[]".to_vec()).await;
        let err = result.expect_err("4xx must surface as error");
        match err {
            TransportError::HttpStatus { status, .. } => assert_eq!(status, 422),
            other => panic!("expected HttpStatus 422; got {other:?}"),
        }
        assert_eq!(client.calls(), 1, "4xx must not be retried");
        assert!(
            sleeper.sleeps().is_empty(),
            "no sleep when classification is terminal",
        );
    }

    #[tokio::test]
    async fn retry_does_not_retry_on_401() {
        // Regression pin: 401 lives in 4xx and must not be retried.
        let client = ScriptedClient::new(vec![http_401()]);
        let sleeper = RecordingSleeper::new();
        let result =
            post_json_with_retry(&client, &policy(3), &sleeper, "/v1/events", b"[]".to_vec()).await;
        assert!(result.is_err());
        assert_eq!(client.calls(), 1, "401 must not be retried");
    }

    #[tokio::test]
    async fn retry_does_not_retry_on_config_error() {
        let client = ScriptedClient::new(vec![config_err()]);
        let sleeper = RecordingSleeper::new();
        let result =
            post_json_with_retry(&client, &policy(3), &sleeper, "/v1/events", b"[]".to_vec()).await;
        let err = result.expect_err("Config must surface as error");
        assert!(matches!(err, TransportError::Config(_)));
        assert_eq!(client.calls(), 1, "Config errors must not be retried");
        assert!(sleeper.sleeps().is_empty());
    }

    #[tokio::test]
    async fn retry_backoff_doubles_base_delay_between_attempts() {
        // Three attempts → two sleeps. With base_delay = 100ms, the
        // schedule must be [100ms, 200ms]. Pinning the schedule
        // protects against mutations that swap *= 2 for += base or
        // similar arithmetic regressions.
        let client = ScriptedClient::new(vec![http_503(), http_503(), http_503()]);
        let sleeper = RecordingSleeper::new();
        let _ =
            post_json_with_retry(&client, &policy(3), &sleeper, "/v1/events", b"[]".to_vec()).await;
        let sleeps = sleeper.sleeps();
        assert_eq!(sleeps.len(), 2, "two sleeps for three attempts");
        assert_eq!(
            sleeps[0],
            Duration::from_millis(100),
            "first wait is base_delay",
        );
        assert_eq!(
            sleeps[1],
            Duration::from_millis(200),
            "second wait is 2 × base_delay",
        );
    }

    #[tokio::test]
    async fn retry_respects_max_attempts_of_one_no_retries() {
        // max_attempts = 1 ⇒ single attempt, zero sleeps even on 5xx.
        let client = ScriptedClient::new(vec![http_503()]);
        let sleeper = RecordingSleeper::new();
        let _ =
            post_json_with_retry(&client, &policy(1), &sleeper, "/v1/events", b"[]".to_vec()).await;
        assert_eq!(client.calls(), 1);
        assert!(
            sleeper.sleeps().is_empty(),
            "max_attempts=1 disables retry; no sleeps allowed",
        );
    }

    #[tokio::test]
    async fn retry_surfaces_terminal_error_after_transient_retries() {
        // 503 → 422: the retry kicks in once, then 422 is terminal so the
        // loop exits with the 422 (not the 503).
        let client = ScriptedClient::new(vec![http_503(), http_422()]);
        let sleeper = RecordingSleeper::new();
        let result =
            post_json_with_retry(&client, &policy(3), &sleeper, "/v1/events", b"[]".to_vec()).await;
        let err = result.expect_err("422 after retry must surface");
        match err {
            TransportError::HttpStatus { status, .. } => assert_eq!(
                status, 422,
                "terminal-after-transient must surface the terminal error",
            ),
            other => panic!("expected HttpStatus 422; got {other:?}"),
        }
        assert_eq!(client.calls(), 2, "one retry then terminal stop");
        assert_eq!(
            sleeper.sleeps().len(),
            1,
            "exactly one sleep before the terminal attempt",
        );
    }
}
