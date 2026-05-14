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
use std::future::Future;
use std::num::NonZeroU32;
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
/// `Send + Sync` on the trait AND `+ Send` on the returned future —
/// future Task 4b `--stream` work shares a client across tokio tasks on
/// a multi-thread runtime; the Send bound is mechanical to add now and
/// painful to retrofit later. Pinned by
/// `retry_works_under_multi_thread_runtime` in `retry_tests`.
pub trait HttpClient: Send + Sync {
    fn post_json(
        &self,
        path: &str,
        body: Vec<u8>,
    ) -> impl Future<Output = Result<Value, TransportError>> + Send;
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
/// `max_attempts` counts the TOTAL attempts. The `NonZeroU32` lifts the
/// "at least one attempt" invariant into the type system — a runtime
/// `expect` in the retry loop's exhaustion branch would otherwise sit on
/// a pub API.
///
/// Backoff: before attempt `M` (for `M >= 2`), sleep
/// `min(base_delay * 2^(M-2), max_delay)`. Concrete schedule for
/// `base=100ms`: 100ms before attempt 2, 200ms before attempt 3, 400ms
/// before attempt 4, … capped at `max_delay`. No jitter in v1 — added
/// when production data justifies it.
///
/// `max_delay` exists because `max_attempts` is `pub` and a future
/// caller might pick a large value; without a cap, doubling `base_delay`
/// reaches multi-hour sleeps within ~20 attempts. The cap is also
/// `pub` so callers with different SLAs can override it (e.g. a `--max-
/// retry-delay` flag on a future `nt publish`).
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: NonZeroU32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl RetryPolicy {
    /// Production defaults: 3 attempts, 100ms base delay (matches the
    /// TS reference's 100ms → 200ms schedule for idempotent GETs), 30s
    /// per-attempt cap.
    ///
    /// Honours `NT_RETRY_BASE_DELAY_MS` for test-side speed-ups. When
    /// the integration suite sets it to "0", the retry loop still
    /// observably retries (sleep call records still fire, classifier
    /// still runs) but each test pays ~0ms instead of 100–300ms of
    /// real wall-clock. Production callers never set this; documented
    /// in `tests/publish.rs` next to the run_nt_publish helper.
    pub fn default_publish() -> Self {
        let base_delay = std::env::var("NT_RETRY_BASE_DELAY_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .map(Duration::from_millis)
            .unwrap_or(Duration::from_millis(100));
        Self {
            max_attempts: NonZeroU32::new(3).expect("3 is non-zero"),
            base_delay,
            max_delay: Duration::from_secs(30),
        }
    }
}

/// Abstract clock seam — production wires `TokioSleeper`; tests inject a
/// recording fake that returns immediately and captures the requested
/// durations so backoff schedule can be asserted without real waits.
///
/// `+ Send` on the returned future for the same reason as `HttpClient` —
/// retry-under-multi-thread-runtime is a pinned invariant.
pub trait Sleeper: Send + Sync {
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send;
}

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
/// Backoff: before attempt `M` (`M >= 2`), sleeps
/// `min(base_delay * 2^(M-2), max_delay)`. For `base=100ms`,
/// `max_delay=30s`: 100ms before attempt 2, 200ms before attempt 3,
/// 400ms before 4, … capped at 30s. No sleep after the final attempt —
/// give-up returns synchronously after the last transport call.
///
/// Takes `body: &[u8]` rather than `Vec<u8>` — the function clones to
/// `Vec<u8>` per attempt (the trait owns the buffer once handed off to
/// reqwest), and a borrowed signature is honest about that pattern. A
/// future caller batching megabytes can promote to `Arc<Vec<u8>>` if
/// the per-attempt clone shows up in a profile; envelope-sized bodies
/// today (< 4 KB) are dwarfed by the network round trip.
pub async fn post_json_with_retry<C: HttpClient, S: Sleeper>(
    client: &C,
    policy: &RetryPolicy,
    sleeper: &S,
    path: &str,
    body: &[u8],
) -> Result<Value, TransportError> {
    // Tracks the most-recent transient error so we can surface it after
    // the budget exhausts. Terminal errors short-circuit, so this only
    // ever holds the last *retriable* failure.
    let mut last_transient: Option<TransportError> = None;
    let max_attempts = policy.max_attempts.get();
    for attempt in 1..=max_attempts {
        if attempt > 1 {
            // Exponential backoff: base * 2^(attempt-2), capped at
            // max_delay. `checked_pow` saturates to `u32::MAX` at
            // `attempt - 2 >= 32` so the multiplication can't panic in
            // debug or silently wrap in release; `Duration::saturating_
            // mul` further saturates to `Duration::MAX` so the
            // `.min(max_delay)` below is the operative cap.
            let multiplier = 2u32.checked_pow(attempt - 2).unwrap_or(u32::MAX);
            let delay = policy
                .base_delay
                .saturating_mul(multiplier)
                .min(policy.max_delay);
            sleeper.sleep(delay).await;
        }
        match client.post_json(path, body.to_vec()).await {
            Ok(value) => return Ok(value),
            Err(err) if is_transient(&err) => {
                last_transient = Some(err);
                continue;
            }
            // Terminal error — short-circuit even if a previous
            // transient retry happened. Caller sees the terminal cause,
            // not the earlier transient one.
            Err(err) => return Err(err),
        }
    }
    // Budget exhausted. The loop only exits here via the transient
    // branch (which sets `last_transient`) because `NonZeroU32`
    // guarantees `max_attempts >= 1` so at least one iteration runs.
    Err(last_transient.expect(
        "retry loop ran at least once per NonZeroU32 invariant on RetryPolicy::max_attempts",
    ))
}

/// Classifies a TransportError as retriable. Pure: no I/O, no clock.
/// Exposed for the retry loop's branching and for direct test coverage.
pub fn is_transient(err: &TransportError) -> bool {
    match err {
        // Network failures (DNS, TCP, TLS handshake, timeout) — the
        // canonical retriable class. Reqwest's `is_timeout` / `is_connect`
        // could refine this further once the structured-error contract
        // lands; v1 retries every network-class error.
        TransportError::Network(_) => true,
        // 5xx are server-side transients; 4xx are client-side terminals
        // and re-sending won't change the outcome. The 500 boundary is
        // pinned by `is_transient_classifies_500_boundary_as_transient`
        // and the 499 boundary by `is_transient_classifies_499_boundary_as_terminal`.
        TransportError::HttpStatus { status, .. } => *status >= 500,
        // Config errors are caller misuse (bad URL, malformed body) —
        // retrying re-runs the same broken inputs against the same
        // server. Never retry.
        TransportError::Config(_) => false,
    }
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
            max_attempts: NonZeroU32::new(max_attempts)
                .expect("policy() helper rejects 0 — non-zero invariant lives in the type"),
            // Tests assert on relative ratios via the RecordingSleeper.
            // The absolute value is irrelevant because the fake returns
            // synchronously.
            base_delay: Duration::from_millis(100),
            // Large enough that no `policy(N)` test brushes against the
            // cap. `max_delay` coverage lives in its own test below.
            max_delay: Duration::from_secs(3600),
        }
    }

    /// Fabricates a real `reqwest::Error` (the only path to constructing
    /// a `TransportError::Network`, since `reqwest::Error` has no public
    /// constructor). Uses a short-timeout request to a reserved-closed
    /// port; expected to fail with connect-refused or timeout. The
    /// ~50ms wall-clock cost is paid once per test that needs it.
    async fn make_network_error() -> TransportError {
        let err = reqwest::Client::new()
            .get("http://127.0.0.1:1/")
            .timeout(Duration::from_millis(50))
            .send()
            .await
            .expect_err("port 1 is reserved + closed; must fail to connect");
        TransportError::Network(err)
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
        // 499 lives in its own boundary test below — it carries the
        // better failure message and we don't want two assertions for
        // the same input.
        for code in [400u16, 401, 403, 404, 422] {
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

    #[tokio::test]
    async fn is_transient_classifies_network_as_transient() {
        // Pins the canonical retriable class. A regression flipping the
        // Network match arm from true to false would otherwise pass the
        // suite — caught explicitly here.
        let err = make_network_error().await;
        assert!(
            is_transient(&err),
            "Network errors must be classified transient (got {err})",
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
        let result = post_json_with_retry(&client, &policy(3), &sleeper, "/v1/events", b"[]").await;
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
        let result = post_json_with_retry(&client, &policy(3), &sleeper, "/v1/events", b"[]").await;
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
        let result = post_json_with_retry(&client, &policy(3), &sleeper, "/v1/events", b"[]").await;
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
        let result = post_json_with_retry(&client, &policy(3), &sleeper, "/v1/events", b"[]").await;
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
        let result = post_json_with_retry(&client, &policy(3), &sleeper, "/v1/events", b"[]").await;
        assert!(result.is_err());
        assert_eq!(client.calls(), 1, "401 must not be retried");
    }

    #[tokio::test]
    async fn retry_does_not_retry_on_config_error() {
        let client = ScriptedClient::new(vec![config_err()]);
        let sleeper = RecordingSleeper::new();
        let result = post_json_with_retry(&client, &policy(3), &sleeper, "/v1/events", b"[]").await;
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
        let _ = post_json_with_retry(&client, &policy(3), &sleeper, "/v1/events", b"[]").await;
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
        let _ = post_json_with_retry(&client, &policy(1), &sleeper, "/v1/events", b"[]").await;
        assert_eq!(client.calls(), 1);
        assert!(
            sleeper.sleeps().is_empty(),
            "max_attempts=1 disables retry; no sleeps allowed",
        );
    }

    #[tokio::test]
    async fn retry_retries_network_error_then_succeeds() {
        // Pairs with `is_transient_classifies_network_as_transient` —
        // proves the retry loop actually re-attempts on the Network
        // branch, not just that the classifier says it's transient.
        let net_err = make_network_error().await;
        let client = ScriptedClient::new(vec![Err(net_err), ok_body()]);
        let sleeper = RecordingSleeper::new();
        let result = post_json_with_retry(&client, &policy(3), &sleeper, "/v1/events", b"[]").await;
        assert!(
            result.is_ok(),
            "network-error then 200 must surface the 2xx success: {result:?}",
        );
        assert_eq!(client.calls(), 2, "exactly one retry after network error");
        assert_eq!(
            sleeper.sleeps().len(),
            1,
            "one sleep between the two attempts",
        );
    }

    #[tokio::test]
    async fn retry_backoff_caps_at_max_delay() {
        // Pin the max_delay clamp: with base=100ms doubling, attempt 30
        // would otherwise compute ~13.4 years. Cap at 1s; assert no
        // recorded sleep exceeds it.
        let attempts = 30u32;
        let responses: Vec<_> = (0..attempts).map(|_| http_503()).collect();
        let client = ScriptedClient::new(responses);
        let sleeper = RecordingSleeper::new();
        let policy = RetryPolicy {
            max_attempts: NonZeroU32::new(attempts).unwrap(),
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(1),
        };
        let _ = post_json_with_retry(&client, &policy, &sleeper, "/v1/events", b"[]").await;
        let sleeps = sleeper.sleeps();
        for (i, d) in sleeps.iter().enumerate() {
            assert!(
                *d <= Duration::from_secs(1),
                "sleep[{i}] = {d:?} exceeded max_delay 1s",
            );
        }
        // Sanity: at least one sleep DID hit the cap (otherwise the
        // test is silently exercising the un-capped path).
        assert!(
            sleeps.iter().any(|d| *d == Duration::from_secs(1)),
            "at least one sleep should hit the cap; got {sleeps:?}",
        );
    }

    #[tokio::test]
    async fn retry_backoff_does_not_panic_at_attempt_overflow_boundary() {
        // attempt - 2 == 32 would overflow `2u32.pow`. Use a small
        // base_delay + small max_delay to keep wall-clock zero
        // (RecordingSleeper returns immediately anyway) and assert the
        // loop completes without panic. Regression pin for
        // `2u32.checked_pow(...).unwrap_or(u32::MAX)`.
        let attempts = 35u32;
        let responses: Vec<_> = (0..attempts).map(|_| http_503()).collect();
        let client = ScriptedClient::new(responses);
        let sleeper = RecordingSleeper::new();
        let policy = RetryPolicy {
            max_attempts: NonZeroU32::new(attempts).unwrap(),
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(10),
        };
        let result = post_json_with_retry(&client, &policy, &sleeper, "/v1/events", b"[]").await;
        assert!(result.is_err(), "all-503 must surface error");
        assert_eq!(client.calls(), attempts as usize);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn retry_works_under_multi_thread_runtime() {
        // Locks the `+ Send` bound on the returned futures of `Sleeper`
        // and `HttpClient` — under a multi-thread runtime tokio requires
        // Send-ness across await points. A regression that drops the
        // Send bound would fail compilation here.
        let client = ScriptedClient::new(vec![http_503(), ok_body()]);
        let sleeper = RecordingSleeper::new();
        let result = post_json_with_retry(&client, &policy(3), &sleeper, "/v1/events", b"[]").await;
        assert!(result.is_ok());
        assert_eq!(client.calls(), 2);
    }

    #[tokio::test]
    async fn retry_surfaces_terminal_error_after_transient_retries() {
        // 503 → 422: the retry kicks in once, then 422 is terminal so the
        // loop exits with the 422 (not the 503).
        let client = ScriptedClient::new(vec![http_503(), http_422()]);
        let sleeper = RecordingSleeper::new();
        let result = post_json_with_retry(&client, &policy(3), &sleeper, "/v1/events", b"[]").await;
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

    #[tokio::test]
    async fn tokio_sleeper_actually_sleeps_for_requested_duration() {
        // Pins the real-clock body of `TokioSleeper::sleep` against
        // mutations that strip it to `()`. Without this, the suite
        // only ever exercises `RecordingSleeper` and `NoopSleeper`
        // (both no-op for testability) — production-binary backoff
        // could regress to no-wait without any test signal.
        //
        // 50ms is short enough not to slow the suite meaningfully and
        // long enough to absorb scheduler jitter on slow CI runners.
        let sleeper = TokioSleeper;
        let start = std::time::Instant::now();
        sleeper.sleep(Duration::from_millis(50)).await;
        let elapsed = start.elapsed();
        assert!(
            elapsed >= Duration::from_millis(40),
            "real sleep must elapse near the requested duration; got {elapsed:?}",
        );
    }
}
