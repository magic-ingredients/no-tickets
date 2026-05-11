//! Minimal HTTPS client for the publish spike. Wraps `reqwest` with
//! Bearer-auth header injection, JSON body serialisation, and structured
//! error mapping. Mirrors `src/transport/client.ts::request` at the
//! happy-path level. Retries / backoff / ETag / streaming are out of
//! scope for the spike (Task 5 will own those).

use std::fmt;

use serde::Serialize;
use serde_json::Value;

#[derive(Debug)]
pub enum TransportError {
    /// Network-level failure (DNS, TCP, TLS, timeout, etc.).
    Network(String),
    /// Server responded with a non-2xx status. Carries the status code
    /// and the raw response body so callers can render server-side
    /// validation messages verbatim.
    HttpStatus { status: u16, body: String },
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportError::Network(msg) => {
                write!(f, "transport error: {msg}")
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
    base_url: String,
    token: String,
}

impl Client {
    pub fn new(base_url: String, token: String) -> Result<Self, TransportError> {
        let inner = reqwest::Client::builder()
            .build()
            .map_err(|e| TransportError::Network(e.to_string()))?;
        Ok(Self {
            inner,
            base_url,
            token,
        })
    }

    /// POST `path` (relative to `base_url`) with the given JSON-serialisable
    /// body. Returns the response body as a `Value` on 2xx; an
    /// `HttpStatus` error otherwise. Server response body bytes are
    /// preserved verbatim in the error variant so structured error
    /// messages (validation issues, unknown-type names, etc.) survive
    /// to stderr.
    pub async fn post_json<T: Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<Value, TransportError> {
        let url = join_url(&self.base_url, path);
        let response = self
            .inner
            .post(&url)
            .bearer_auth(&self.token)
            .header("content-type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| TransportError::Network(e.to_string()))?;

        let status = response.status();
        let body_text = response
            .text()
            .await
            .map_err(|e| TransportError::Network(e.to_string()))?;

        if !status.is_success() {
            return Err(TransportError::HttpStatus {
                status: status.as_u16(),
                body: body_text,
            });
        }

        if body_text.is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&body_text)
            .map_err(|e| TransportError::Network(format!("invalid server JSON: {e}")))
    }
}

fn join_url(base: &str, path: &str) -> String {
    let base_trimmed = base.trim_end_matches('/');
    let path_prefixed = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    format!("{base_trimmed}{path_prefixed}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_url_strips_trailing_base_slash() {
        assert_eq!(join_url("https://api.example/", "/v1/events"), "https://api.example/v1/events");
        assert_eq!(join_url("https://api.example", "/v1/events"), "https://api.example/v1/events");
    }

    #[test]
    fn join_url_adds_leading_path_slash() {
        assert_eq!(join_url("https://api.example", "v1/events"), "https://api.example/v1/events");
    }
}
