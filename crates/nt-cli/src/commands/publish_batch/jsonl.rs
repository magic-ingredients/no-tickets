//! JSONL input handling: read from file/stdin and parse newline-
//! delimited JSON into line-numbered entries.

use serde_json::Value;
use tokio::io::AsyncReadExt;

/// One parsed JSONL line, with its 1-based source line number for
/// diagnostic messages.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct JsonlEntry {
    pub(super) line: usize,
    pub(super) value: Value,
}

/// Parse a JSONL string into a vec of `JsonlEntry`. Blank lines (incl.
/// lines containing only a trailing CR) are skipped. Parse failures
/// report the failing 1-based line number.
///
/// Pure: no I/O. Trailing-CR stripping handles Windows-saved JSONL —
/// `\r\n` line endings would otherwise leave a stray `\r` at the end
/// of each line and fail `serde_json::from_str` with an unhelpful
/// "expected EOF after value" message.
pub(super) fn parse_jsonl(input: &str) -> Result<Vec<JsonlEntry>, String> {
    let mut result = Vec::new();
    for (i, raw) in input.split('\n').enumerate() {
        // 1-based line numbering — matches the way every editor and
        // every error tool counts lines, and matches the TS reference.
        let line_number = i + 1;
        // `str::trim` strips ASCII whitespace including `\r`, so a
        // trailing CR from Windows CRLF is removed alongside any other
        // padding. The earlier explicit `trim_end_matches('\r')` was
        // redundant against `trim()` and made the call read like the
        // two had distinct semantics.
        let line = raw.trim();
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

/// Read JSONL input from a file path or stdin (`-`). I/O-only; the
/// returned string is then passed to `parse_jsonl`. File-read failures
/// surface as a user-facing error string naming the path.
///
/// Memory: reads the entire input into a `String` before parsing. For
/// the canonical batch size (hundreds to low thousands of envelopes,
/// each a few KB), this fits comfortably in memory. Matches the TS
/// reference (`src/cli/lib/jsonl.ts::readJsonl` reads the whole file
/// with `readFileSync`). A future "stream-and-parse line-by-line"
/// optimisation could land if a real workload pushed the bound, but
/// the single-POST wire contract already caps a sensible batch size
/// at whatever the server accepts in one body — so streaming the
/// input wouldn't change the upper bound on memory anyway.
pub(super) async fn read_batch_input(batch_path: &str) -> Result<String, String> {
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

    #[test]
    fn parse_jsonl_scalar_lines_are_accepted_at_parse_layer() {
        // Same contract as the array-line test: any valid JSON value
        // parses successfully here; the per-line shape filter
        // (`validate_and_build_envelope`) is what rejects scalars and
        // arrays with a "line N: expected an object event" message.
        // Pinned per scalar type because a future "is_object early"
        // optimisation in parse_jsonl could otherwise drift the
        // contract on only some shapes.
        for (line, predicate) in [
            ("null", Value::is_null as fn(&Value) -> bool),
            ("42", Value::is_number),
            ("true", Value::is_boolean),
            ("\"a string\"", Value::is_string),
        ] {
            let result = parse_jsonl(line).expect("scalar line parses at JSON layer");
            assert_eq!(result.len(), 1);
            assert!(
                predicate(&result[0].value),
                "scalar {line:?} should parse to its native JSON shape; got {:?}",
                result[0].value,
            );
        }
    }
}
