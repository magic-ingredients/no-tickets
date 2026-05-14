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

use nt_schemas::validate;
use serde_json::{Map, Value};
use tokio::io::AsyncReadExt;

use crate::auth::{emit_host_mismatch_warning, resolve_auth, AuthOutcome, NOT_AUTH_MSG};
use crate::env::Env;
use crate::source_detect::machine_hash_attribute;
use crate::transport::{
    post_json_with_retry, Client, HttpClient, RetryPolicy, Sleeper, TokioSleeper,
};
use crate::urls::resolve_urls;

use super::publish::parse_source_attribute;

/// One parsed JSONL line, with its 1-based source line number for
/// diagnostic messages.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonlEntry {
    pub line: usize,
    pub value: Value,
}

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
pub async fn run(args: PublishBatchArgs<'_>, env: &dyn Env) -> i32 {
    // 1. Read raw input from file or stdin.
    let raw = match read_batch_input(args.batch_path).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };

    // 2. Parse JSONL — line-numbered errors point to the source file.
    let entries = match parse_jsonl(&raw) {
        Ok(es) => es,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };

    // 3. Empty batch is a usage error, not a no-op success.
    if entries.is_empty() {
        eprintln!("batch file \"{}\" is empty", args.batch_path);
        return 1;
    }

    // 4. Compute the CLI base source once for the whole batch. Machine
    //    hash is resolved here (same as single-event) so the entire
    //    batch attributes the same producing machine. Per-line source
    //    overrides merge on top of this base.
    let machine_hash_owned: Option<String> = machine_hash_attribute(env);
    let cli_source = match build_cli_base_source(
        args.source_name,
        args.project,
        args.source_attributes,
        machine_hash_owned.as_deref(),
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };

    // 5. Per-line validation + envelope construction. Any failure
    //    short-circuits with a line-numbered diagnostic and exit 1.
    let mut envelopes: Vec<Value> = Vec::with_capacity(entries.len());
    for entry in entries {
        match validate_and_build_envelope(&entry, &cli_source) {
            Ok(envelope) => envelopes.push(envelope),
            Err(msg) => {
                eprintln!("{msg}");
                return 1;
            }
        }
    }

    // 6. Resolve URLs + auth (same shape as single-event run()).
    let urls = match resolve_urls(env) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("{}", e.user_message());
            return 1;
        }
    };
    let auth = match resolve_auth(env, &urls.api_url) {
        AuthOutcome::Resolved(a) => a,
        AuthOutcome::SessionHostMismatch {
            stored_host,
            current_host,
        } => {
            emit_host_mismatch_warning(&stored_host, &current_host);
            eprintln!("{NOT_AUTH_MSG}");
            return 1;
        }
        AuthOutcome::None => {
            eprintln!("{NOT_AUTH_MSG}");
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

    // 7. Single POST with the batch array; reuses retry/backoff from
    //    Task 17. Body serialises a `Vec<Value>` so each line's full
    //    envelope (incl. merged source) lands on the wire verbatim.
    let body_bytes = serde_json::to_vec(&envelopes).expect("envelope vec always serialises");
    let policy = RetryPolicy::default_publish();
    let sleeper = TokioSleeper;
    publish_envelopes(&client, &policy, &sleeper, &body_bytes).await
}

/// Post the serialised batch envelope array, print server response,
/// map to an exit code. Mirrors `commands::publish::publish_event` but
/// for a multi-envelope body.
async fn publish_envelopes<C: HttpClient, S: Sleeper>(
    client: &C,
    policy: &RetryPolicy,
    sleeper: &S,
    body_bytes: &[u8],
) -> i32 {
    match post_json_with_retry(client, policy, sleeper, "/v1/events", body_bytes).await {
        Ok(response) => {
            println!(
                "{}",
                serde_json::to_string(&response).expect("serde_json::Value always serialises"),
            );
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

/// Validate one entry (shape, type, schema) and produce the envelope
/// with the merged source. Returns the envelope as a `Value` or a
/// user-facing error string with the line number prepended.
fn validate_and_build_envelope(entry: &JsonlEntry, cli_source: &Value) -> Result<Value, String> {
    let obj = entry
        .value
        .as_object()
        .ok_or_else(|| format!("line {}: expected an object event", entry.line))?;
    let type_id = obj
        .get("type")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("line {}: missing or empty \"type\" field", entry.line))?;

    let data = obj.get("data").cloned().unwrap_or(Value::Null);
    match validate(type_id, &data) {
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

/// Parse a JSONL string into a vec of `JsonlEntry`. Blank lines (incl.
/// lines containing only a trailing CR) are skipped. Parse failures
/// report the failing 1-based line number.
///
/// Pure: no I/O. Trailing-CR stripping handles Windows-saved JSONL —
/// `\r\n` line endings would otherwise leave a stray `\r` at the end
/// of each line and fail `serde_json::from_str` with an unhelpful
/// "expected EOF after value" message.
pub fn parse_jsonl(input: &str) -> Result<Vec<JsonlEntry>, String> {
    let mut result = Vec::new();
    for (i, raw) in input.split('\n').enumerate() {
        // 1-based line numbering — matches the way every editor and
        // every error tool counts lines, and matches the TS reference.
        let line_number = i + 1;
        // Strip trailing CR (Windows CRLF) then surrounding whitespace.
        // A line that's all whitespace (incl. a lone CR) is treated as
        // blank — no phantom entry on trailing newline.
        let line = raw.trim_end_matches('\r').trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(line)
            .map_err(|e| format!("JSONL parse error on line {line_number}: {e}"))?;
        result.push(JsonlEntry {
            line: line_number,
            value,
        });
    }
    Ok(result)
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
pub fn merge_source(cli_source: &Value, jsonl_source: Option<&Value>) -> Value {
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
pub fn build_cli_base_source(
    source_name: Option<&str>,
    project: &str,
    flag_attributes: &[String],
    machine_hash: Option<&str>,
) -> Result<Value, String> {
    // Insertion order: project → machine hash → flag attributes.
    // The flag attributes go last so an explicit `--source-attribute
    // machine=X` overrides the auto-computed value (mirrors single-
    // event semantics).
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
        Value::String(source_name.unwrap_or("nt-cli").to_string()),
    );
    source.insert(
        "sdkVersion".to_string(),
        Value::String(env!("CARGO_PKG_VERSION").to_string()),
    );
    source.insert("attributes".to_string(), Value::Object(attrs));
    Ok(Value::Object(source))
}

/// Read JSONL input from a file path or stdin (`-`). I/O-only; the
/// returned string is then passed to `parse_jsonl`. File-read failures
/// surface as a user-facing error string naming the path.
pub async fn read_batch_input(batch_path: &str) -> Result<String, String> {
    if batch_path == "-" {
        let mut buf = String::new();
        tokio::io::stdin()
            .read_to_string(&mut buf)
            .await
            .map_err(|e| format!("failed to read stdin: {e}"))?;
        Ok(buf)
    } else {
        std::fs::read_to_string(batch_path)
            .map_err(|e| format!("could not read JSONL file \"{batch_path}\": {e}"))
    }
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
