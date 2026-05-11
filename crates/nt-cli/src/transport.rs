//! Minimal HTTPS client for the publish spike. Wraps `reqwest` with
//! Bearer-auth header injection, JSON body serialisation, and structured
//! error mapping. Mirrors `src/transport/client.ts::request` at the
//! happy-path level. Retries / backoff / ETag / streaming are out of
//! scope for the spike (Task 5 will own those).
//!
//! Error variants are minimal in v1 (Config / Network / HttpStatus).
//! Task 5a's 7-exit-code structured-error contract will refine this —
//! the spike preserves enough information (full server body, reqwest
//! error chain, original config-failure message) that the future
//! mapping is straightforward and lossless.

use std::fmt;
use std::time::Duration;

use serde::Serialize;
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
    async fn post_json(
        &self,
        path: &str,
        body: Vec<u8>,
    ) -> Result<Value, TransportError>;
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

    /// POST `path` (relative to `base_url`) with the given JSON-serialisable
    /// body. Returns the response body as a `Value` on 2xx; an
    /// `HttpStatus` error otherwise.
    pub async fn post_json<T: Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<Value, TransportError> {
        let url = self
            .base_url
            .join(path)
            .map_err(|e| TransportError::Config(format!("invalid path {path:?}: {e}")))?;
        let response = self
            .inner
            .post(url)
            .bearer_auth(&self.token)
            // .json() sets Content-Type: application/json itself; no
            // explicit .header(...) needed.
            .json(body)
            .send()
            .await
            .map_err(TransportError::Network)?;

        let status = response.status();
        let body_text = response
            .text()
            .await
            .map_err(TransportError::Network)?;

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
