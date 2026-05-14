//! Best-effort JSON Schema → example-payload synthesiser.
//!
//! Rust port of `src/lib/example-synth.ts`. Mirrors the resolution
//! order from the TS reference:
//!   1. `default` (if the schema node declares one)
//!   2. first `enum` value (when present and non-empty)
//!   3. type-driven placeholder (empty string, 0, false, null)
//!
//! Recurses into objects (every declared property is synthesised) and
//! arrays (single placeholder element from `items`). Unknown shapes
//! collapse to `Value::Null` — the trust boundary accepts any
//! `serde_json::Value`, including primitives and arrays at the top
//! level, and returns null when the input isn't a JSON object.
//!
//! Used by the `describe_event_type` MCP tool to produce a starter
//! payload alongside the JSON Schema. RED-phase stub returns null for
//! every input; GREEN fills in the resolution branches.

use serde_json::Value;

/// Synthesise a minimal valid example payload from a JSON Schema
/// fragment. Returns `Value::Null` for malformed inputs.
///
/// `#[allow(dead_code)]` at RED — production wire-up lives in
/// `tools/describe_event_type.rs::handle` (also RED-stubbed), so the
/// dead-code lint fires until GREEN connects the two.
#[allow(dead_code)]
pub fn synthesise_example(_raw_schema: &Value) -> Value {
    // RED stub — replaced at GREEN with the full default → enum → type
    // placeholder cascade. Returning Null here makes every behaviour
    // assertion in the unit tests fail predictably.
    Value::Null
}

#[cfg(test)]
mod tests {
    use super::synthesise_example;
    use serde_json::{json, Value};

    // ── Primitives ────────────────────────────────────────────────

    #[test]
    fn primitive_string_yields_empty_string() {
        assert_eq!(synthesise_example(&json!({ "type": "string" })), json!(""));
    }

    #[test]
    fn primitive_number_yields_zero() {
        assert_eq!(synthesise_example(&json!({ "type": "number" })), json!(0));
    }

    #[test]
    fn primitive_integer_yields_zero() {
        assert_eq!(synthesise_example(&json!({ "type": "integer" })), json!(0));
    }

    #[test]
    fn primitive_boolean_yields_false() {
        assert_eq!(
            synthesise_example(&json!({ "type": "boolean" })),
            json!(false),
        );
    }

    #[test]
    fn primitive_null_yields_null() {
        assert_eq!(synthesise_example(&json!({ "type": "null" })), Value::Null);
    }

    // ── Defaults beat type placeholders ───────────────────────────

    #[test]
    fn default_string_beats_type_placeholder() {
        assert_eq!(
            synthesise_example(&json!({ "type": "string", "default": "hello" })),
            json!("hello"),
        );
    }

    #[test]
    fn default_number_beats_type_placeholder() {
        assert_eq!(
            synthesise_example(&json!({ "type": "number", "default": 42 })),
            json!(42),
        );
    }

    #[test]
    fn default_false_is_used_even_though_type_default_is_false() {
        // Pin: a declared `default: false` must be honoured rather than
        // skipped because it's falsy. Catches a regression where the
        // implementation checks truthiness instead of presence.
        assert_eq!(
            synthesise_example(&json!({ "type": "boolean", "default": false })),
            json!(false),
        );
    }

    #[test]
    fn default_null_is_used_even_though_type_placeholder_is_null() {
        // Same idea as the `default: false` test — a declared
        // `default: null` is still a real declaration and must NOT be
        // confused with "no default present". A regression using
        // `is_null` to detect absence would surface as a fallthrough to
        // the type placeholder; since the placeholder here is also null
        // the test pins the principle on a `type: string` node where
        // the two paths diverge.
        assert_eq!(
            synthesise_example(&json!({ "type": "string", "default": null })),
            Value::Null,
        );
    }

    // ── Enums ─────────────────────────────────────────────────────

    #[test]
    fn enum_first_value_used_when_no_default() {
        assert_eq!(
            synthesise_example(&json!({ "type": "string", "enum": ["a", "b", "c"] })),
            json!("a"),
        );
    }

    #[test]
    fn default_beats_enum_first_value() {
        assert_eq!(
            synthesise_example(&json!({
                "type": "string", "enum": ["a", "b"], "default": "b"
            })),
            json!("b"),
        );
    }

    #[test]
    fn enum_without_type_still_resolves_to_first_value() {
        assert_eq!(
            synthesise_example(&json!({ "enum": ["x", "y"] })),
            json!("x"),
        );
    }

    #[test]
    fn empty_enum_falls_through_to_type_placeholder() {
        assert_eq!(
            synthesise_example(&json!({ "type": "string", "enum": [] })),
            json!(""),
        );
        assert_eq!(
            synthesise_example(&json!({ "type": "integer", "enum": [] })),
            json!(0),
        );
    }

    // ── Objects ───────────────────────────────────────────────────

    #[test]
    fn object_without_properties_yields_empty_object() {
        assert_eq!(
            synthesise_example(&json!({ "type": "object" })),
            json!({}),
        );
    }

    #[test]
    fn object_synthesises_every_property() {
        assert_eq!(
            synthesise_example(&json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "age": { "type": "integer" }
                }
            })),
            json!({ "name": "", "age": 0 }),
        );
    }

    #[test]
    fn object_respects_per_property_defaults_and_enums() {
        assert_eq!(
            synthesise_example(&json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "default": "Ada" },
                    "plan": { "type": "string", "enum": ["free", "pro"] }
                }
            })),
            json!({ "name": "Ada", "plan": "free" }),
        );
    }

    #[test]
    fn object_recurses_into_nested_objects() {
        assert_eq!(
            synthesise_example(&json!({
                "type": "object",
                "properties": {
                    "user": {
                        "type": "object",
                        "properties": {
                            "email": { "type": "string", "default": "a@b.c" }
                        }
                    }
                }
            })),
            json!({ "user": { "email": "a@b.c" } }),
        );
    }

    // ── Arrays ────────────────────────────────────────────────────

    #[test]
    fn array_with_items_yields_single_synthesised_element() {
        assert_eq!(
            synthesise_example(&json!({
                "type": "array",
                "items": { "type": "string" }
            })),
            json!([""]),
        );
    }

    #[test]
    fn array_without_items_yields_empty_array() {
        assert_eq!(
            synthesise_example(&json!({ "type": "array" })),
            json!([]),
        );
    }

    // ── Trust-boundary fallbacks ──────────────────────────────────

    #[test]
    fn empty_object_schema_yields_null() {
        // No `type`, no `enum`, no `default` → wholly unknown shape →
        // null. Without this, a {} schema would silently produce `{}`
        // which would mislead the agent into thinking the type wants
        // an empty object payload.
        assert_eq!(synthesise_example(&json!({})), Value::Null);
    }

    #[test]
    fn unrecognised_type_yields_null() {
        assert_eq!(
            synthesise_example(&json!({ "type": "lambda-soup" })),
            Value::Null,
        );
    }

    #[test]
    fn primitive_input_yields_null() {
        assert_eq!(synthesise_example(&json!("not-a-schema")), Value::Null);
        assert_eq!(synthesise_example(&json!(42)), Value::Null);
        assert_eq!(synthesise_example(&json!(true)), Value::Null);
    }

    #[test]
    fn null_input_yields_null() {
        assert_eq!(synthesise_example(&Value::Null), Value::Null);
    }

    #[test]
    fn array_input_yields_null() {
        // The trust boundary accepts a Value; only objects qualify as
        // a schema node. Arrays at the top level collapse to null.
        assert_eq!(
            synthesise_example(&json!([{ "type": "string" }])),
            Value::Null,
        );
    }
}
