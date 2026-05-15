//! Thin reqwest wrappers providing Bearer-authenticated JSON
//! GET / POST primitives. Returns `nt_core::Error` for all failures;
//! consumers map to their own error types.
//!
//! No retry. No backoff. No status-code semantic mapping (the
//! `HttpStatus` variant carries the raw `{status, body}` and the
//! consumer decides what 401/403/404/5xx mean in its context).
//!
//! Successful (2xx) responses return:
//! - `get_json` / `post_json` → parsed `serde_json::Value`
//! - `get_raw` / `post_raw` → the raw response body as a `String`
//!   plus its status, when the caller wants the body even on 2xx
//!   (e.g. nt-mcp's publish_event reads the body to extract the
//!   server-returned event id).

use reqwest::Client;
use serde_json::Value;

use crate::error::Error;

/// GET `url` with `Bearer {token}`; return the parsed JSON body on
/// 2xx, an `Error::HttpStatus { status, body }` on non-2xx.
pub async fn get_json(client: &Client, url: &str, token: &str) -> Result<Value, Error> {
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| Error::Transport(e.to_string()))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| Error::Body(e.to_string()))?;
    if !status.is_success() {
        return Err(Error::HttpStatus {
            status: status.as_u16(),
            body,
        });
    }
    serde_json::from_str(&body).map_err(|e| Error::InvalidJson(e.to_string()))
}

/// GET `url` with `Bearer {token}`; return the raw `{status, body}`
/// pair (no JSON parse, no error on non-2xx — caller decides). For
/// the rare case where a consumer wants to react to status codes
/// without paying the parse cost on 4xx bodies.
#[allow(dead_code)]
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

/// POST `url` with a JSON `body`, `Bearer {token}`, `content-type:
/// application/json`. Returns the raw `{status, body}` so callers
/// can inspect non-2xx server messages verbatim (nt-mcp publishes
/// surface the upstream body in error text) AND parse 2xx bodies
/// themselves (the response shape varies per endpoint).
pub async fn post_json(
    client: &Client,
    url: &str,
    token: &str,
    body: &Value,
) -> Result<RawResponse, Error> {
    let response = client
        .post(url)
        .bearer_auth(token)
        .header("content-type", "application/json")
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

/// Status + body pair returned by the raw helpers. `status` is
/// `u16` rather than `reqwest::StatusCode` so callers can match
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
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client() -> Client {
        Client::builder().build().expect("build reqwest client")
    }

    #[tokio::test]
    async fn get_json_returns_parsed_body_on_2xx() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/x"))
            .and(header("authorization", "Bearer tok"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"hello": "world"})))
            .expect(1)
            .mount(&server)
            .await;
        let url = format!("{}/v1/x", server.uri());
        let v = get_json(&client(), &url, "tok").await.expect("ok");
        assert_eq!(v["hello"], "world");
    }

    #[tokio::test]
    async fn get_json_returns_http_status_on_non_2xx() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/x"))
            .respond_with(ResponseTemplate::new(404).set_body_string("not here"))
            .expect(1)
            .mount(&server)
            .await;
        let url = format!("{}/v1/x", server.uri());
        match get_json(&client(), &url, "tok").await {
            Err(Error::HttpStatus { status, body }) => {
                assert_eq!(status, 404);
                assert_eq!(body, "not here");
            }
            other => panic!("expected HttpStatus(404); got {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_json_returns_invalid_json_on_non_json_2xx_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/x"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not-json"))
            .expect(1)
            .mount(&server)
            .await;
        let url = format!("{}/v1/x", server.uri());
        match get_json(&client(), &url, "tok").await {
            Err(Error::InvalidJson(msg)) => {
                assert!(!msg.is_empty(), "InvalidJson must carry a message");
            }
            other => panic!("expected InvalidJson; got {other:?}"),
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
        // Unlike `get_json`, `post_json` does NOT convert non-2xx
        // into `Err(HttpStatus)` — callers want to inspect the
        // status + body themselves (the typed-error mapping is
        // consumer-specific).
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
}
