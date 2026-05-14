//! `nt publish --file <path>` — batch publish from JSONL.
//!
//! Mirrors `src/cli/commands/publish/batch.ts::runPublishBatch` from the
//! TS reference. Reads JSONL (one JSON object per line) from a file path
//! (or stdin when path is `-`), validates each line locally, builds
//! one envelope per line with a per-line source override on top of the
//! CLI base source, and sends the lot as a single POST to `/v1/events`.
//!
//! Distinct from Task 4b (`--stream` mode): batch is one finite read
//! → one HTTP call → exit. Stream is a long-lived subprocess with
//! JSONL on stdin AND stdout.

use serde_json::Value;

use crate::env::Env;

/// One parsed JSONL line, with its 1-based source line number for
/// diagnostic messages.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonlEntry {
    pub line: usize,
    pub value: Value,
}

#[allow(dead_code)] // Fields read by Task 16 GREEN.
pub struct PublishBatchArgs<'a> {
    /// Path to a `.jsonl` file, or `-` to read from stdin.
    pub batch_path: &'a str,
    /// Project name; appears in `source.attributes.project` on every
    /// envelope in the batch (matches single-event behaviour).
    pub project: &'a str,
    /// Override the default `source.name` ("nt-cli") on the CLI base
    /// source. JSONL lines may override per-line via their own
    /// `source.name`.
    pub source_name: Option<&'a str>,
    /// `--source-attribute KEY=VALUE` entries to seed
    /// `source.attributes` on every envelope. JSONL line attributes
    /// merge on top (line wins on key collisions).
    pub source_attributes: &'a [String],
}

/// Entry point. Reads input, parses JSONL, validates per line, merges
/// sources, sends the batch, prints the response, returns an exit code.
#[allow(dead_code)] // Wired by Task 16 GREEN into main.rs.
pub async fn run(_args: PublishBatchArgs<'_>, _env: &dyn Env) -> i32 {
    unimplemented!("Task 16 GREEN — batch publish from JSONL file or stdin")
}

/// Parse a JSONL string into a vec of `JsonlEntry`. Blank lines (incl.
/// lines containing only a trailing CR) are skipped. Parse failures
/// report the failing 1-based line number.
///
/// Pure: no I/O. Trailing-CR stripping handles Windows-saved JSONL —
/// `\r\n` line endings would otherwise leave a stray `\r` at the end
/// of each line and fail `serde_json::from_str` with an unhelpful
/// "expected EOF after value" message.
#[allow(dead_code)] // Wired by Task 16 GREEN; exercised inline.
pub fn parse_jsonl(_input: &str) -> Result<Vec<JsonlEntry>, String> {
    unimplemented!("Task 16 GREEN — JSONL parser with line-numbered errors")
}

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
#[allow(dead_code)] // Wired by Task 16 GREEN; exercised inline.
pub fn merge_source(_cli_source: &Value, _jsonl_source: Option<&Value>) -> Value {
    unimplemented!("Task 16 GREEN — shallow source merge with attributes key-merge")
}

/// Build an `nt-cli`-base source value from the CLI inputs. Mirrors
/// the per-event source the single-event path constructs in
/// `commands::publish::build_metadata` plus the machine-hash
/// attribute when opted in. Pure given its inputs.
#[allow(dead_code)] // Wired by Task 16 GREEN.
pub fn build_cli_base_source(
    _source_name: Option<&str>,
    _project: &str,
    _flag_attributes: &[String],
    _machine_hash: Option<&str>,
) -> Result<Value, String> {
    unimplemented!("Task 16 GREEN — assemble CLI base source from flags")
}

/// Read JSONL input from a file path or stdin (`-`). I/O-only; the
/// returned string is then passed to `parse_jsonl`. File-read failures
/// surface as a user-facing error string naming the path.
#[allow(dead_code)] // Wired by Task 16 GREEN.
pub async fn read_batch_input(_batch_path: &str) -> Result<String, String> {
    unimplemented!("Task 16 GREEN — read JSONL from file or stdin")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ─── parse_jsonl ────────────────────────────────────────────────────

    #[test]
    fn parse_jsonl_empty_string_returns_empty_vec() {
        let result = parse_jsonl("").expect("empty input is not an error");
        assert!(
            result.is_empty(),
            "empty input must yield zero entries; got {result:?}",
        );
    }

    #[test]
    fn parse_jsonl_single_line_produces_one_entry_at_line_1() {
        let result = parse_jsonl(r#"{"a":1}"#).expect("valid one-liner");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].line, 1, "line numbering is 1-based");
        assert_eq!(result[0].value, json!({"a": 1}));
    }

    #[test]
    fn parse_jsonl_two_lines_produce_entries_with_sequential_line_numbers() {
        let input = "{\"a\":1}\n{\"b\":2}\n";
        let result = parse_jsonl(input).expect("valid two-liner");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].line, 1);
        assert_eq!(result[1].line, 2);
        assert_eq!(result[0].value, json!({"a": 1}));
        assert_eq!(result[1].value, json!({"b": 2}));
    }

    #[test]
    fn parse_jsonl_blank_lines_are_skipped_and_dont_consume_line_numbers() {
        // Blank lines (incl. whitespace-only) skipped, BUT subsequent
        // entries keep their 1-based original-source line number — so
        // error messages point to the right line in the source file.
        let input = "{\"a\":1}\n\n\n{\"b\":2}\n";
        let result = parse_jsonl(input).expect("blank lines OK");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].line, 1);
        assert_eq!(
            result[1].line, 4,
            "second entry must report its actual source line (4), not its position (2)",
        );
    }

    #[test]
    fn parse_jsonl_trailing_cr_is_stripped_for_windows_jsonl() {
        // Windows-saved JSONL uses CRLF; without stripping the trailing
        // \r, serde_json fails to parse with an unhelpful "expected
        // EOF after value" message. Pin the strip so the parser is
        // platform-independent.
        let input = "{\"a\":1}\r\n{\"b\":2}\r\n";
        let result = parse_jsonl(input).expect("CRLF must be tolerated");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].value, json!({"a": 1}));
        assert_eq!(result[1].value, json!({"b": 2}));
    }

    #[test]
    fn parse_jsonl_invalid_json_reports_line_number() {
        let input = "{\"a\":1}\nthis is not json\n{\"b\":2}\n";
        let err = parse_jsonl(input).expect_err("line 2 is malformed");
        assert!(
            err.contains("line 2"),
            "error must name the failing line number; got {err:?}",
        );
    }

    #[test]
    fn parse_jsonl_no_trailing_newline_still_parses_last_entry() {
        let input = "{\"a\":1}\n{\"b\":2}"; // no trailing \n
        let result = parse_jsonl(input).expect("no-trailing-newline OK");
        assert_eq!(
            result.len(),
            2,
            "missing trailing newline must not drop last entry"
        );
    }

    #[test]
    fn parse_jsonl_array_line_is_accepted_at_parse_layer() {
        // The parser is shape-agnostic — `validate_entry` is responsible
        // for rejecting non-object lines. Pin this so a future "early
        // shape filter in parse_jsonl" change can't drift the contract.
        let input = "[1, 2, 3]\n";
        let result = parse_jsonl(input).expect("array line parses at JSON layer");
        assert!(result[0].value.is_array());
    }

    // ─── merge_source ───────────────────────────────────────────────────

    #[test]
    fn merge_source_returns_cli_source_unchanged_when_no_jsonl_override() {
        let cli =
            json!({ "name": "nt-cli", "sdkVersion": "1.0", "attributes": { "project": "demo" } });
        let merged = merge_source(&cli, None);
        assert_eq!(merged, cli);
    }

    #[test]
    fn merge_source_returns_cli_source_unchanged_when_jsonl_source_is_non_object() {
        // Malformed per-line source is silently ignored — matches TS.
        let cli = json!({ "name": "nt-cli", "sdkVersion": "1.0" });
        let merged = merge_source(&cli, Some(&json!("a string, not an object")));
        assert_eq!(merged, cli);
    }

    #[test]
    fn merge_source_jsonl_top_level_fields_override_cli() {
        let cli = json!({ "name": "nt-cli", "sdkVersion": "1.0" });
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
            "name": "nt-cli",
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
        let cli = json!({ "name": "nt-cli" });
        let jsonl = json!({ "attributes": { "k": "v" } });
        let merged = merge_source(&cli, Some(&jsonl));
        assert_eq!(merged["attributes"]["k"], "v");
    }

    #[test]
    fn merge_source_omits_attributes_when_neither_side_provides_any() {
        let cli = json!({ "name": "nt-cli", "sdkVersion": "1.0" });
        let jsonl = json!({ "name": "override" });
        let merged = merge_source(&cli, Some(&jsonl));
        assert!(
            merged.get("attributes").is_none(),
            "no attributes anywhere → no `attributes` key on the merged source; got {merged}",
        );
    }

    // ─── build_cli_base_source ──────────────────────────────────────────

    #[test]
    fn build_cli_base_source_minimal_inputs_yields_name_and_project() {
        let result = build_cli_base_source(None, "demo", &[], None).expect("valid");
        assert_eq!(result["name"], "nt-cli");
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
