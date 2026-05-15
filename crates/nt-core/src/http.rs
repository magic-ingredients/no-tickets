//! Thin reqwest wrappers providing Bearer-authenticated GET / POST
//! primitives. Returns `nt_core::Error` for transport / body /
//! parse failures; non-2xx HTTP responses come back as a
//! `RawResponse { status, body }` so callers can map status codes
//! to their own typed errors without paying a parse cost on 4xx
//! bodies.
//!
//! No retry, no backoff, no status-code semantic mapping (404 / 401
//! / 403 / 5xx mean different things in nt-cli vs each nt-mcp tool;
//! each consumer decides).
//!
//! ## Client lifecycle / timeout
//!
//! `get_raw` / `post_json` take a `&reqwest::Client` from the
//! caller. nt-core does NOT construct the client and does NOT
//! impose a timeout — that's the caller's choice (nt-mcp's
//! `NtServer::new()` builds the client with a 30s timeout matching
//! nt-cli's `DEFAULT_TIMEOUT`; tests build a default un-timeouted
//! client because they run against a local wiremock). A request
//! timeout surfaces as `Error::Transport` with the reqwest message
//! text.

use reqwest::Client;

use crate::error::Error;

/// GET `url` with `Bearer {token}`; return the raw `{status, body}`
/// pair. Non-2xx is NOT converted to `Err(HttpStatus)` — callers
/// inspect `response.status` to do their own mapping (nt-mcp tools
/// distinguish 404 / 401-403 / other-non-2xx and emit tool-specific
/// `McpError` wording).
///
/// **The response body is read unconditionally**, even for non-2xx
/// statuses. This is a minor behavioural drift from the pre-
/// extraction shape in `describe_event_type` (which checked status
/// first and only read the body for the generic non-2xx fallback,
/// skipping the read on 401/403/404). The new shape is simpler and
/// the body bytes on those error paths are tiny in practice — the
/// trade-off is intentional. If a caller ever needs to skip the
/// body read for a specific status class, it can drop down to
/// reqwest directly.
pub async fn get_raw(client: &Client, url: &str, token: &str) -> Result<RawResponse, Error> {
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| Error::Transport(e.to_string()))?;
    let status = response.status().as_u16();
    let body = response
        .text()
        .await
        .map_err(|e| Error::Body(e.to_string()))?;
    Ok(RawResponse { status, body })
}

/// POST `url` with a JSON-serialisable `body` and `Bearer {token}`.
/// Same `RawResponse` contract as `get_raw` — non-2xx is returned
/// to the caller for mapping.
///
/// `reqwest::RequestBuilder::json()` sets `Content-Type:
/// application/json` automatically, so no explicit header injection
/// is needed.
pub async fn post_json<B: serde::Serialize + ?Sized>(
    client: &Client,
    url: &str,
    token: &str,
    body: &B,
) -> Result<RawResponse, Error> {
    let response = client
        .post(url)
        .bearer_auth(token)
        .json(body)
        .send()
        .await
        .map_err(|e| Error::Transport(e.to_string()))?;
    let status = response.status().as_u16();
    let body = response
        .text()
        .await
        .map_err(|e| Error::Body(e.to_string()))?;
    Ok(RawResponse { status, body })
}

/// Status + body pair returned by the helpers. `status` is `u16`
/// rather than `reqwest::StatusCode` so callers can match
/// numerically without needing a reqwest dep.
#[derive(Debug, Clone)]
pub struct RawResponse {
    pub status: u16,
    pub body: String,
}

impl RawResponse {
    /// True for 2xx statuses.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{header, header_exists, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client() -> Client {
        Client::builder().build().expect("build reqwest client")
    }

    #[tokio::test]
    async fn get_raw_returns_status_and_body_on_2xx() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/x"))
            .and(header("authorization", "Bearer tok"))
            .respond_with(ResponseTemplate::new(200).set_body_string("hello"))
            .expect(1)
            .mount(&server)
            .await;
        let url = format!("{}/v1/x", server.uri());
        let resp = get_raw(&client(), &url, "tok").await.expect("ok");
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, "hello");
        assert!(resp.is_success());
    }

    #[tokio::test]
    async fn get_raw_returns_raw_response_on_non_2xx() {
        // Non-2xx is NOT an Err. Caller maps it.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/x"))
            .respond_with(ResponseTemplate::new(404).set_body_string("not here"))
            .expect(1)
            .mount(&server)
            .await;
        let url = format!("{}/v1/x", server.uri());
        let resp = get_raw(&client(), &url, "tok")
            .await
            .expect("transport ok even on 404");
        assert_eq!(resp.status, 404);
        assert_eq!(resp.body, "not here");
        assert!(!resp.is_success());
    }

    #[tokio::test]
    async fn get_raw_transport_error_on_connect_refused() {
        // Port 1 is reserved + closed — reqwest fails before any
        // status/body is read. Pins the Transport variant against a
        // real connect-refused so a regression collapsing
        // Transport into Body or HttpStatus is caught.
        let resp = get_raw(&client(), "http://127.0.0.1:1/x", "tok").await;
        match resp {
            Err(Error::Transport(_)) => {}
            other => panic!("expected Transport; got {other:?}"),
        }
    }

    #[tokio::test]
    async fn post_json_returns_raw_response_on_2xx() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/events"))
            .and(header("authorization", "Bearer tok"))
            .and(header("content-type", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ids": ["evt_1"]})))
            .expect(1)
            .mount(&server)
            .await;
        let url = format!("{}/v1/events", server.uri());
        let resp = post_json(&client(), &url, "tok", &json!([{"type": "x"}]))
            .await
            .expect("ok");
        assert_eq!(resp.status, 200);
        assert!(resp.body.contains("evt_1"));
        assert!(resp.is_success());
    }

    #[tokio::test]
    async fn post_json_returns_raw_response_on_non_2xx() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/events"))
            .respond_with(ResponseTemplate::new(503).set_body_string("upstream down"))
            .expect(1)
            .mount(&server)
            .await;
        let url = format!("{}/v1/events", server.uri());
        let resp = post_json(&client(), &url, "tok", &json!([]))
            .await
            .expect("transport ok even though server 503'd");
        assert_eq!(resp.status, 503);
        assert_eq!(resp.body, "upstream down");
        assert!(!resp.is_success());
    }

    #[tokio::test]
    async fn post_json_sets_content_type_application_json_exactly_once() {
        // `reqwest::RequestBuilder::json()` injects the
        // content-type header itself; we used to inject it manually
        // too, which produced a multi-value header on the wire.
        // Pin the single-header behaviour so a regression that
        // re-adds the manual `.header(...)` call is caught.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/x"))
            .and(header_exists("content-type"))
            .and(header("content-type", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(""))
            .expect(1)
            .mount(&server)
            .await;
        let url = format!("{}/x", server.uri());
        let _ = post_json(&client(), &url, "tok", &json!({})).await.unwrap();
    }
}
