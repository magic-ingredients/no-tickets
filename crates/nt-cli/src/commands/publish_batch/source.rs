//! CLI-base source assembly + per-line merge for batch publish.
//!
//! `build_cli_base_source` produces the per-batch source object from
//! the `--source-name`, `--source-attribute KEY=VALUE`, and
//! `NO_TICKETS_INCLUDE_MACHINE` inputs. `merge_source` layers an
//! optional per-line JSONL `source` field on top, with JSONL winning
//! on top-level field collisions and key-merging the `attributes` map.

use serde_json::{Map, Value};

use crate::commands::publish::{parse_source_attribute, DEFAULT_SOURCE_NAME, SDK_VERSION};

/// Merge a per-line JSONL source override onto the CLI base source.
/// JSONL top-level fields win on collision; `attributes` is key-merged
/// with the JSONL line's attributes on top of the CLI's.
///
/// `cli_source` is always a JSON object (the caller built it from the
/// flag inputs). `jsonl_source` may be `None` (line had no source
/// field), `Some(Value::Object)` (line carried a source override),
/// or `Some(non-object)` (line carried a malformed source — silently
/// ignored to match TS behaviour, since the per-line source field is
/// optional and a wrong shape there shouldn't reject the whole event).
pub(super) fn merge_source(cli_source: &Value, jsonl_source: Option<&Value>) -> Value {
    // Caller invariant: `cli_source` is always a JSON object — it's
    // produced by `build_cli_base_source`, which only returns objects.
    // Defensive `as_object()` rather than `.expect` so a future caller
    // that breaks the invariant degrades gracefully (return cli as-is).
    let Some(cli_obj) = cli_source.as_object() else {
        return cli_source.clone();
    };
    let jsonl_obj = jsonl_source.and_then(Value::as_object);
    let Some(jsonl_obj) = jsonl_obj else {
        // No JSONL override (None) OR malformed line-source (non-object).
        // Either way the CLI base survives verbatim — matches TS
        // (a non-object `source` field on a JSONL line is silently
        // ignored rather than failing the whole event).
        return Value::Object(cli_obj.clone());
    };

    let mut merged: Map<String, Value> = cli_obj.clone();
    // JSONL top-level fields override CLI on collision, but `attributes`
    // is handled separately below (key-merge, not overwrite).
    for (k, v) in jsonl_obj {
        if k == "attributes" {
            continue;
        }
        merged.insert(k.clone(), v.clone());
    }

    // Attributes key-merge: CLI's keys first, then JSONL's on top.
    let cli_attrs = cli_obj.get("attributes").and_then(Value::as_object);
    let jsonl_attrs = jsonl_obj.get("attributes").and_then(Value::as_object);
    match (cli_attrs, jsonl_attrs) {
        (None, None) => {
            // Neither side has attributes — omit the key entirely so
            // the wire body doesn't carry an empty `attributes: {}`.
            merged.remove("attributes");
        }
        (cli, jsonl) => {
            let mut attrs = Map::new();
            if let Some(c) = cli {
                for (k, v) in c {
                    attrs.insert(k.clone(), v.clone());
                }
            }
            if let Some(j) = jsonl {
                for (k, v) in j {
                    attrs.insert(k.clone(), v.clone());
                }
            }
            merged.insert("attributes".to_string(), Value::Object(attrs));
        }
    }

    Value::Object(merged)
}

/// Build an `nt-cli`-base source value from the CLI inputs. Mirrors
/// the per-event source the single-event path constructs in
/// `commands::publish::build_metadata` plus the machine-hash
/// attribute when opted in. Pure given its inputs.
pub(super) fn build_cli_base_source(
    source_name: Option<&str>,
    project: &str,
    flag_attributes: &[String],
    machine_hash: Option<&str>,
) -> Result<Value, String> {
    // Insert order matters only for last-write-wins on the `machine`
    // key: flag attributes go AFTER the auto-computed machine hash so
    // an explicit `--source-attribute machine=X` overwrites the auto
    // value. Map iteration order on serialisation is independent
    // (`serde_json::Map` without `preserve_order` enumerates keys
    // alphabetically) — that's fine for the wire contract, which is
    // a JSON object whose key order is not significant.
    let mut attrs = Map::new();
    attrs.insert("project".to_string(), Value::String(project.to_string()));
    if let Some(hash) = machine_hash {
        attrs.insert("machine".to_string(), Value::String(hash.to_string()));
    }
    for raw in flag_attributes {
        let (k, v) = parse_source_attribute(raw)?;
        attrs.insert(k.to_string(), Value::String(v.to_string()));
    }

    let mut source = Map::new();
    source.insert(
        "name".to_string(),
        Value::String(source_name.unwrap_or(DEFAULT_SOURCE_NAME).to_string()),
    );
    source.insert(
        "sdkVersion".to_string(),
        Value::String(SDK_VERSION.to_string()),
    );
    source.insert("attributes".to_string(), Value::Object(attrs));
    Ok(Value::Object(source))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ─── merge_source ───────────────────────────────────────────────────

    #[test]
    fn merge_source_returns_cli_source_unchanged_when_no_jsonl_override() {
        let cli = json!({ "name": "no-tickets", "sdkVersion": "1.0", "attributes": { "project": "demo" } });
        let merged = merge_source(&cli, None);
        assert_eq!(merged, cli);
    }

    #[test]
    fn merge_source_returns_cli_source_unchanged_when_jsonl_source_is_non_object() {
        // Malformed per-line source is silently ignored — matches TS.
        let cli = json!({ "name": "no-tickets", "sdkVersion": "1.0" });
        let merged = merge_source(&cli, Some(&json!("a string, not an object")));
        assert_eq!(merged, cli);
    }

    #[test]
    fn merge_source_jsonl_top_level_fields_override_cli() {
        let cli = json!({ "name": "no-tickets", "sdkVersion": "1.0" });
        let jsonl = json!({ "name": "custom-publisher" });
        let merged = merge_source(&cli, Some(&jsonl));
        assert_eq!(merged["name"], "custom-publisher");
        assert_eq!(
            merged["sdkVersion"], "1.0",
            "fields the JSONL didn't supply must come from the CLI base",
        );
    }

    #[test]
    fn merge_source_attributes_are_key_merged_with_jsonl_on_top() {
        let cli = json!({
            "name": "no-tickets",
            "attributes": { "project": "demo", "ci": "github-actions" }
        });
        let jsonl = json!({
            "attributes": { "ci": "buildkite", "extra": "value" }
        });
        let merged = merge_source(&cli, Some(&jsonl));
        let attrs = &merged["attributes"];
        assert_eq!(attrs["project"], "demo", "CLI-only key survives");
        assert_eq!(attrs["ci"], "buildkite", "JSONL wins on key collision");
        assert_eq!(attrs["extra"], "value", "JSONL-only key included");
    }

    #[test]
    fn merge_source_attributes_jsonl_alone_become_the_merged_attributes() {
        // CLI source has no attributes; JSONL provides them. Result
        // carries the JSONL attributes verbatim.
        let cli = json!({ "name": "no-tickets" });
        let jsonl = json!({ "attributes": { "k": "v" } });
        let merged = merge_source(&cli, Some(&jsonl));
        assert_eq!(merged["attributes"]["k"], "v");
    }

    #[test]
    fn merge_source_omits_attributes_when_neither_side_provides_any() {
        let cli = json!({ "name": "no-tickets", "sdkVersion": "1.0" });
        let jsonl = json!({ "name": "override" });
        let merged = merge_source(&cli, Some(&jsonl));
        assert!(
            merged.get("attributes").is_none(),
            "no attributes anywhere → no `attributes` key on the merged source; got {merged}",
        );
    }

    #[test]
    fn merge_source_keeps_cli_attributes_when_jsonl_source_has_no_attributes_key() {
        // (Some, None) branch in the attributes match. The reviewer
        // flagged that the `(None, None) → remove("attributes")` arm
        // is only meaningfully different from the catch-all when CLI
        // contributes attributes AND JSONL doesn't. Pin both halves:
        // the resulting source must carry the CLI's project + ci, and
        // the `attributes` key must NOT be removed.
        let cli = json!({
            "name": "no-tickets",
            "attributes": { "project": "demo", "ci": "github-actions" }
        });
        let jsonl = json!({ "name": "custom-runner" });
        let merged = merge_source(&cli, Some(&jsonl));
        assert_eq!(merged["name"], "custom-runner", "JSONL top-level wins");
        let attrs = &merged["attributes"];
        assert_eq!(attrs["project"], "demo", "CLI attribute survives");
        assert_eq!(attrs["ci"], "github-actions", "CLI attribute survives");
    }

    #[test]
    fn merge_source_treats_non_object_cli_attributes_as_absent() {
        // Defensive path: cli_obj.get("attributes").and_then(Value::as_object)
        // returns None if the value is non-object (string, number, etc.).
        // Pin that this drops CLI attrs entirely rather than panicking
        // or somehow merging a non-object into the result.
        let cli = json!({ "name": "no-tickets", "attributes": "not an object" });
        let jsonl = json!({ "attributes": { "k": "v" } });
        let merged = merge_source(&cli, Some(&jsonl));
        // Only the JSONL attribute survives; the bogus CLI value is dropped.
        assert_eq!(merged["attributes"]["k"], "v");
        assert!(
            merged["attributes"].get("project").is_none(),
            "non-object CLI attrs must NOT leak into merged.attributes",
        );
    }

    #[test]
    fn merge_source_treats_non_object_jsonl_attributes_as_absent() {
        // Symmetric to the above: non-object JSONL `attributes` is
        // silently ignored. CLI attrs survive verbatim.
        let cli = json!({
            "name": "no-tickets",
            "attributes": { "project": "demo" }
        });
        let jsonl = json!({ "attributes": 42 });
        let merged = merge_source(&cli, Some(&jsonl));
        assert_eq!(merged["attributes"]["project"], "demo");
    }

    // ─── build_cli_base_source ──────────────────────────────────────────

    #[test]
    fn build_cli_base_source_minimal_inputs_yields_name_and_project() {
        let result = build_cli_base_source(None, "demo", &[], None).expect("valid");
        assert_eq!(result["name"], "no-tickets");
        assert_eq!(result["attributes"]["project"], "demo");
    }

    #[test]
    fn build_cli_base_source_source_name_flag_overrides_default() {
        let result =
            build_cli_base_source(Some("custom-publisher"), "demo", &[], None).expect("valid");
        assert_eq!(result["name"], "custom-publisher");
    }

    #[test]
    fn build_cli_base_source_attributes_include_flag_pairs_and_machine_hash() {
        let attrs = ["ci=github-actions".to_string(), "runner=ubuntu".to_string()];
        let result =
            build_cli_base_source(None, "demo", &attrs, Some("abcdef1234567890")).expect("valid");
        let a = &result["attributes"];
        assert_eq!(a["project"], "demo");
        assert_eq!(a["ci"], "github-actions");
        assert_eq!(a["runner"], "ubuntu");
        assert_eq!(a["machine"], "abcdef1234567890");
    }

    #[test]
    fn build_cli_base_source_malformed_flag_attribute_returns_user_error() {
        let attrs = ["no-equals-sign".to_string()];
        let err = build_cli_base_source(None, "demo", &attrs, None).expect_err("malformed");
        assert!(
            err.to_lowercase().contains("source-attribute") || err.contains("key=value"),
            "error must name the malformed flag; got {err:?}",
        );
    }

    #[test]
    fn build_cli_base_source_flag_attribute_overrides_machine_hash_when_keys_collide() {
        // Matches the single-event ordering: machine hash is the
        // "automatic" value; an explicit `--source-attribute machine=X`
        // must win.
        let attrs = ["machine=manual-override".to_string()];
        let result =
            build_cli_base_source(None, "demo", &attrs, Some("auto-computed")).expect("valid");
        assert_eq!(result["attributes"]["machine"], "manual-override");
    }
}
