//! Per-line validation + envelope construction.
//!
//! Validates one parsed JSONL entry (shape, type, schema) and produces
//! the envelope with the per-line `source` merged onto the CLI base.
//! Returns the envelope as a `Value` or a user-facing error string
//! with the line number prepended.

use nt_schemas::validate;
use serde_json::Value;

use super::jsonl::JsonlEntry;
use super::source::merge_source;

pub(super) fn validate_and_build_envelope(
    entry: &JsonlEntry,
    cli_source: &Value,
) -> Result<Value, String> {
    let obj = entry
        .value
        .as_object()
        .ok_or_else(|| format!("line {}: expected an object event", entry.line))?;
    let type_id = obj
        .get("type")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("line {}: missing or empty \"type\" field", entry.line))?;

    // `validate` takes `&Value`, so the missing-`data` case borrows a
    // module-scoped `Value::Null` rather than allocating a fresh one
    // per entry. Saves a `.clone()` on every batch line.
    static NULL: Value = Value::Null;
    let data: &Value = obj.get("data").unwrap_or(&NULL);
    match validate(type_id, data) {
        None => {
            return Err(format!(
                "line {}: unknown event type \"{}\"",
                entry.line, type_id
            ))
        }
        Some(issues) if !issues.is_empty() => {
            // Multi-line error: header + indented per-issue path/message.
            // Newlines inside the returned String survive `eprintln!`.
            let mut msg = format!("line {}: {} validation error(s):", entry.line, issues.len());
            for issue in &issues {
                msg.push_str(&format!("\n  {}: {}", issue.path, issue.message));
            }
            return Err(msg);
        }
        Some(_) => {}
    }

    // Build the envelope: clone the line verbatim, overwrite `source`
    // with the merged result. Other envelope-level keys (subject,
    // parentEventId, traceId, dedupeKey, ...) flow through unchanged
    // from the JSONL line — that's the per-line metadata contract.
    let mut envelope = entry.value.clone();
    let envelope_obj = envelope.as_object_mut().expect("validated as object above");
    let jsonl_source = envelope_obj.get("source").cloned();
    let merged = merge_source(cli_source, jsonl_source.as_ref());
    envelope_obj.insert("source".to_string(), merged);
    Ok(envelope)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn neutral_cli_source() -> Value {
        // Minimal valid CLI base source for the validate_and_build_envelope
        // tests — concrete shape doesn't matter beyond being an object;
        // these tests pin per-line validation, not merge behaviour.
        serde_json::json!({
            "name": "nt-cli",
            "attributes": { "project": "demo" }
        })
    }

    #[test]
    fn validate_and_build_envelope_rejects_array_line_with_line_number() {
        let entry = JsonlEntry {
            line: 7,
            value: serde_json::json!([1, 2, 3]),
        };
        let err = validate_and_build_envelope(&entry, &neutral_cli_source())
            .expect_err("array line is not an event");
        assert!(
            err.contains("line 7"),
            "error must name the line; got {err:?}"
        );
        assert!(
            err.to_lowercase().contains("object"),
            "error must explain the shape requirement; got {err:?}",
        );
    }

    #[test]
    fn validate_and_build_envelope_rejects_scalar_lines_with_line_number() {
        for (label, value) in [
            ("null", Value::Null),
            ("number", serde_json::json!(42)),
            ("bool", serde_json::json!(true)),
            ("string", serde_json::json!("a string")),
        ] {
            let entry = JsonlEntry {
                line: 3,
                value: value.clone(),
            };
            let err = validate_and_build_envelope(&entry, &neutral_cli_source())
                .expect_err("scalar line is not an event");
            assert!(
                err.contains("line 3"),
                "scalar {label}: error must name the line; got {err:?}",
            );
        }
    }

    #[test]
    fn validate_and_build_envelope_rejects_empty_type_id_with_line_number() {
        // Pins the `.filter(|s| !s.is_empty())` step against a mutation
        // that drops the empty-check: type-id `""` would otherwise slip
        // through to nt_schemas::validate as a known-no-match and fail
        // there with an "unknown type" message — semantically the same
        // outcome but a wrong diagnostic.
        let entry = JsonlEntry {
            line: 5,
            value: serde_json::json!({ "type": "", "data": {} }),
        };
        let err = validate_and_build_envelope(&entry, &neutral_cli_source())
            .expect_err("empty type-id is a usage error");
        assert!(err.contains("line 5"));
        assert!(
            err.to_lowercase().contains("type"),
            "error must reference the type field; got {err:?}",
        );
    }

    #[test]
    fn validate_and_build_envelope_rejects_non_string_type_id() {
        let entry = JsonlEntry {
            line: 2,
            value: serde_json::json!({ "type": 42, "data": {} }),
        };
        let err = validate_and_build_envelope(&entry, &neutral_cli_source())
            .expect_err("non-string type-id is a usage error");
        assert!(err.contains("line 2"));
    }

    #[test]
    fn validate_and_build_envelope_reports_unknown_type_with_line_number() {
        let entry = JsonlEntry {
            line: 9,
            value: serde_json::json!({ "type": "not.a.real.type.v999", "data": {} }),
        };
        let err = validate_and_build_envelope(&entry, &neutral_cli_source())
            .expect_err("unknown type rejected");
        assert!(err.contains("line 9"));
        assert!(
            err.contains("not.a.real.type.v999"),
            "error must surface the offending type; got {err:?}",
        );
    }

    #[test]
    fn validate_and_build_envelope_missing_data_field_falls_back_to_null() {
        // Pins the `obj.get("data").unwrap_or(&NULL)` contract: an
        // event line without a `data` field is treated as if it had
        // `data: null`. For a type whose schema requires a populated
        // `data` (e.g. `ai.task.completed.v1` needs `taskId`), this
        // produces a SCHEMA failure (not a missing-field crash). The
        // line number lands on the schema error so the user knows
        // which line to fix.
        let entry = JsonlEntry {
            line: 11,
            value: serde_json::json!({ "type": "ai.task.completed.v1" }),
        };
        let err = validate_and_build_envelope(&entry, &neutral_cli_source())
            .expect_err("missing data → schema validation error");
        assert!(err.contains("line 11"));
        assert!(
            err.to_lowercase().contains("validation error"),
            "missing-data → schema error message; got {err:?}",
        );
    }

    #[test]
    fn validate_and_build_envelope_schema_error_format_pins_header_and_indented_issues() {
        // Pin the multi-line format: header `line N: K validation
        // error(s):` followed by `  path: message` lines. Mutations
        // that change the format (drop the indent, drop the count,
        // re-order) get caught.
        let entry = JsonlEntry {
            line: 4,
            value: serde_json::json!({
                "type": "ai.task.completed.v1",
                "data": { "taskId": 42 } // wrong type — should be string
            }),
        };
        let err = validate_and_build_envelope(&entry, &neutral_cli_source())
            .expect_err("invalid schema data");
        assert!(
            err.starts_with("line 4: "),
            "header must start with `line N: `; got {err:?}",
        );
        assert!(
            err.contains("validation error"),
            "header must contain `validation error`; got {err:?}",
        );
        assert!(
            err.contains("\n  "),
            "issues must be on subsequent lines indented by two spaces; got {err:?}",
        );
    }

    #[test]
    fn validate_and_build_envelope_happy_path_returns_envelope_with_merged_source() {
        let entry = JsonlEntry {
            line: 1,
            value: serde_json::json!({
                "type": "ai.task.completed.v1",
                "data": {
                    "taskId": "t-1",
                    "sessionId": "s-1",
                    "startedAt": "2026-05-01T00:00:00.000Z",
                    "completedAt": "2026-05-01T00:00:01.000Z",
                    "durationMs": 1000,
                    "outcome": "success",
                    "callCount": 1
                },
                "source": { "name": "per-line-override" }
            }),
        };
        let envelope =
            validate_and_build_envelope(&entry, &neutral_cli_source()).expect("valid happy path");
        // Per-line source name wins over the neutral CLI base.
        assert_eq!(envelope["source"]["name"], "per-line-override");
        // Other envelope-level keys flow through unchanged.
        assert_eq!(envelope["type"], "ai.task.completed.v1");
    }
}
