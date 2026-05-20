//! Wire-envelope types and pure builder for a single event.
//!
//! Pure: no I/O, no env reads, no time. Field order on the wire is
//! pinned by `EventEnvelope`'s declaration order (serde_derive emits in
//! declaration order); the tests below assert byte-positions of every
//! envelope-level key in the serialised form.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use super::SDK_VERSION;

/// Serialised event envelope. Field declaration order is preserved by
/// serde derive — `type, data, source, parentEventId?, traceId?,
/// dedupeKey?` — matching the canonical emission order. Every optional
/// field is omitted (not null, not empty string) when unset, via
/// `skip_serializing_if`. The wire-body field-order tests pin this.
///
/// The wire envelope's `subject` slot is retained server-side as a
/// forward-compat slot but neither the CLI nor MCP populate it today;
/// the field is not modelled here until subjects re-enter scope.
#[derive(Serialize)]
pub(super) struct EventEnvelope<'a> {
    #[serde(rename = "type")]
    pub(super) type_id: &'a str,
    pub(super) data: &'a Value,
    pub(super) source: Source<'a>,
    #[serde(rename = "parentEventId", skip_serializing_if = "Option::is_none")]
    pub(super) parent_event_id: Option<&'a str>,
    #[serde(rename = "traceId", skip_serializing_if = "Option::is_none")]
    pub(super) trace_id: Option<&'a str>,
    #[serde(rename = "dedupeKey", skip_serializing_if = "Option::is_none")]
    pub(super) dedupe_key: Option<&'a str>,
}

#[derive(Serialize)]
pub(super) struct Source<'a> {
    pub(super) name: &'a str,
    #[serde(rename = "sdkVersion")]
    pub(super) sdk_version: &'a str,
    /// `attributes` is built as a single ordered map containing the
    /// `project` entry (canonical) plus any `--source-attribute`
    /// overrides. BTreeMap gives a deterministic key order — important
    /// for wire-shape stability and for `last-wins` semantics on
    /// duplicate keys (driven by insert order in `build_metadata`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) attributes: Option<BTreeMap<&'a str, &'a str>>,
}

/// Already-validated metadata for a single event. Construct via
/// `build_metadata`, which pre-merges `--source-attribute` flags into
/// `attributes` (project entry + per-flag overrides, last-wins on
/// duplicate keys).
///
/// `Debug` is required by `Result::expect_err` in the metadata tests.
#[derive(Debug)]
pub(super) struct EventMetadata<'a> {
    pub(super) source_name: &'a str,
    pub(super) attributes: BTreeMap<&'a str, &'a str>,
    pub(super) parent: Option<&'a str>,
    pub(super) trace: Option<&'a str>,
    pub(super) dedupe_key: Option<&'a str>,
}

/// Pure builder for a single event envelope.
///
/// Returns a single `EventEnvelope`, not a one-element Vec — the wire
/// format wraps a single event in `[...]`, but the array shape is a
/// transport concern owned by the caller (`run()` does `vec![envelope]`),
/// not by this builder.
pub(super) fn build_envelope<'a>(
    type_id: &'a str,
    data: &'a Value,
    meta: EventMetadata<'a>,
) -> EventEnvelope<'a> {
    // `attributes` is always non-empty here — `build_metadata` seeds it
    // with the `project` entry on construction and every test path
    // wires through that same constructor. The Option wrapper on
    // `Source::attributes` stays so the type stays open to a future
    // "no attributes at all" branch (e.g. a `--no-source-attributes`
    // flag) without a wire-shape break.
    EventEnvelope {
        type_id,
        data,
        source: Source {
            name: meta.source_name,
            sdk_version: SDK_VERSION,
            attributes: Some(meta.attributes),
        },
        parent_event_id: meta.parent,
        trace_id: meta.trace,
        dedupe_key: meta.dedupe_key,
    }
}

/// Test-only helper: minimal metadata block for envelope-shape and
/// publish-orchestration tests. No flag overrides, default source.name,
/// `project` as the only source attribute. Mirrors what `build_metadata`
/// would produce for a `PublishArgs` with only the three required
/// flags set.
#[cfg(test)]
pub(super) fn bare_meta(project: &str) -> EventMetadata<'_> {
    let mut attributes = BTreeMap::new();
    attributes.insert("project", project);
    EventMetadata {
        // Read from the const rather than re-hardcoding the literal —
        // DEFAULT_SOURCE_NAME's docstring warns about exactly this kind
        // of duplication causing single-vs-batch path drift.
        source_name: super::DEFAULT_SOURCE_NAME,
        attributes,
        parent: None,
        trace: None,
        dedupe_key: None,
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
        let envelope = build_envelope("ai.task.completed.v1", &data, bare_meta(project));
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
        let body = vec![build_envelope(
            "ai.task.completed.v1",
            &data,
            bare_meta("demo"),
        )];
        let wire = serde_json::to_string(&body).expect("serialises");
        assert!(
            wire.starts_with('['),
            "wire body must start with '[': {wire}"
        );
        assert!(wire.ends_with(']'), "wire body must end with ']': {wire}");
        // Exactly one `"type":` key implies exactly one envelope.
        assert_eq!(
            wire.matches(r#""type":"#).count(),
            1,
            "wire body must contain exactly one envelope: {wire}",
        );
    }

    #[test]
    fn build_envelope_source_name_is_no_tickets_cli() {
        let body = serialise_with_neutral_data("demo");
        assert!(
            body.contains(r#""name":"no-tickets-cli""#),
            "source.name must be \"no-tickets-cli\"; got {body}",
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
        let envelope = build_envelope("ai.task.completed.v1", &data, bare_meta("demo"));
        let body = serde_json::to_string(&envelope).expect("serialises");
        assert!(body.contains(r#""taskId":"t-1""#));
        assert!(body.contains(r#""sessionId":"s-1""#));
        assert!(body.contains(r#""nested":{"x":42}"#));
    }

    #[test]
    fn build_envelope_preserves_string_data_payload() {
        let data = Value::String("plain-string-payload".to_string());
        let envelope = build_envelope("ai.task.completed.v1", &data, bare_meta("demo"));
        let body = serde_json::to_string(&envelope).expect("serialises");
        assert!(
            body.contains(r#""data":"plain-string-payload""#),
            "string payload must serialise as-is; got {body}",
        );
    }

    #[test]
    fn build_envelope_preserves_null_data_payload() {
        let data = Value::Null;
        let envelope = build_envelope("ai.task.completed.v1", &data, bare_meta("demo"));
        let body = serde_json::to_string(&envelope).expect("serialises");
        assert!(
            body.contains(r#""data":null"#),
            "null payload must serialise as JSON null; got {body}",
        );
    }

    #[test]
    fn build_envelope_preserves_array_data_payload() {
        let data = json!([1, 2, 3]);
        let envelope = build_envelope("ai.task.completed.v1", &data, bare_meta("demo"));
        let body = serde_json::to_string(&envelope).expect("serialises");
        assert!(
            body.contains(r#""data":[1,2,3]"#),
            "array payload must serialise as-is; got {body}",
        );
    }
}
