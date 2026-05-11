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

    // Wire body is a JSON array. Wrapping the single envelope at the
    // call site keeps build_envelope honest about its scope (one event)
    // and isolates the array shape — a transport concern — to run().
    let body = vec![build_envelope(args.type_id, &parsed_data, args.project)];

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
