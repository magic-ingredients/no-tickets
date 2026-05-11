//! `nt publish` — single-event publish over HTTPS with Bearer auth.
//! Mirrors `src/cli/commands/publish` + `src/transport/events.ts::publish`.
//!
//! Spike scope (Task 14): single event only. No batching, no `--stream`
//! mode, no local schema validation, no source merging beyond
//! auto-fill, no retries. Task 5 (full CLI port) owns the rest.

use serde::Serialize;
use serde_json::Value;

use crate::auth::{NOT_AUTH_MSG, resolve_auth};
use crate::transport::Client;
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

/// Pure builder for the wire-body single-event array. Stub for RED phase;
/// GREEN extracts the literal from `run()` and replaces this body.
fn build_envelope<'a>(
    _type_id: &'a str,
    _data: &'a Value,
    _project: &'a str,
) -> Vec<EventEnvelope<'a>> {
    unimplemented!("build_envelope: extracted in GREEN phase")
}

pub async fn run(args: PublishArgs<'_>) -> i32 {
    // URL resolution first (matches handleStatus pattern). A profile
    // error or partial-pair env-var setup wins over auth missing.
    let urls = match resolve_urls(args.profile) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("{}", e.user_message());
            return 1;
        }
    };

    let Some(auth) = resolve_auth() else {
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

    let envelope = EventEnvelope {
        type_id: args.type_id,
        data: &parsed_data,
        source: Source {
            name: "nt-cli",
            sdk_version: env!("CARGO_PKG_VERSION"),
            attributes: Some(SourceAttributes {
                project: args.project,
            }),
        },
    };
    let body = vec![envelope];

    match client.post_json("/v1/events", &body).await {
        Ok(response) => {
            // Print verbatim. Server response shape:
            // `{ ingested, deduped, ids }`. serde_json::Value
            // serialisation cannot fail for valid Value, so `.expect`
            // is appropriate here.
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn build_envelope_emits_single_element_array() {
        let data = json!({ "taskId": "t-1" });
        let envelope = build_envelope("ai.task.completed.v1", &data, "demo");
        assert_eq!(envelope.len(), 1, "wire body is a single-element JSON array");
    }

    #[test]
    fn build_envelope_field_order_is_type_data_source() {
        let data = json!({ "taskId": "t-1", "sessionId": "s-1" });
        let envelope = build_envelope("ai.task.completed.v1", &data, "demo");
        let body = serde_json::to_string(&envelope).expect("envelope serialises");
        let t = body.find(r#""type":"ai.task.completed.v1""#).expect("type present");
        let d = body.find(r#""data":{"#).expect("data present");
        let s = body.find(r#""source":{"name":"nt-cli""#).expect("source present");
        assert!(
            t < d && d < s,
            "wire field order must be type, data, source — got {body}",
        );
    }

    #[test]
    fn build_envelope_source_name_is_nt_cli() {
        let data = json!({});
        let envelope = build_envelope("ai.task.completed.v1", &data, "demo");
        let body = serde_json::to_string(&envelope).expect("serialises");
        assert!(
            body.contains(r#""name":"nt-cli""#),
            "source.name must be \"nt-cli\"; got {body}",
        );
    }

    #[test]
    fn build_envelope_source_sdk_version_is_crate_version() {
        let data = json!({});
        let envelope = build_envelope("ai.task.completed.v1", &data, "demo");
        let body = serde_json::to_string(&envelope).expect("serialises");
        let expected = format!(r#""sdkVersion":"{}""#, env!("CARGO_PKG_VERSION"));
        assert!(
            body.contains(&expected),
            "source.sdkVersion must match CARGO_PKG_VERSION; got {body}",
        );
    }

    #[test]
    fn build_envelope_writes_project_into_source_attributes() {
        let data = json!({});
        let envelope = build_envelope("ai.task.completed.v1", &data, "my-project");
        let body = serde_json::to_string(&envelope).expect("serialises");
        assert!(
            body.contains(r#""attributes":{"project":"my-project"}"#),
            "source.attributes.project must reflect the input; got {body}",
        );
    }

    #[test]
    fn build_envelope_preserves_data_payload_verbatim() {
        let data = json!({ "taskId": "t-1", "sessionId": "s-1", "nested": { "x": 42 } });
        let envelope = build_envelope("ai.task.completed.v1", &data, "demo");
        let body = serde_json::to_string(&envelope).expect("serialises");
        // taskId/sessionId/nested all present — payload not transformed
        assert!(body.contains(r#""taskId":"t-1""#));
        assert!(body.contains(r#""sessionId":"s-1""#));
        assert!(body.contains(r#""nested":{"x":42}"#));
    }
}
