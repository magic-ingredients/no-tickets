//! Usage validation + `--source-attribute` parsing for `nt publish`.
//!
//! Pure: no I/O. Returns the assembled `EnvelopeInputs` or a user-facing
//! error string. Borrows from `args`, so the returned metadata's
//! lifetime is bounded by the caller's `args`.

use std::collections::BTreeMap;

use super::envelope::EnvelopeInputs;
use super::{PublishArgs, DEFAULT_SOURCE_NAME};

pub(super) fn build_metadata<'a>(
    args: &'a PublishArgs<'a>,
    machine_hash: Option<&'a str>,
) -> Result<EnvelopeInputs<'a>, String> {
    let mut attributes: BTreeMap<&'a str, &'a str> = BTreeMap::new();
    attributes.insert("project", args.project);
    // Insert the auto-computed machine hash BEFORE the flag-derived
    // attribute loop so a `--source-attribute machine=...` flag
    // overrides the auto-value via BTreeMap last-wins on insert.
    // Pinned by `publish_source_attribute_machine_flag_overrides_auto_hash`.
    if let Some(hash) = machine_hash {
        attributes.insert("machine", hash);
    }
    for raw in args.source_attributes {
        let (key, value) = parse_source_attribute(raw)?;
        attributes.insert(key, value);
    }

    Ok(EnvelopeInputs {
        source_name: args.source_name.unwrap_or(DEFAULT_SOURCE_NAME),
        attributes,
        parent: args.parent,
        trace: args.trace,
        dedupe_key: args.dedupe_key,
        // Actor is layered on later, after `actor::resolve` runs in
        // `publish::run`. `build_metadata` doesn't read any actor flags
        // — it's the source/attributes builder, not the actor resolver.
        actor: None,
    })
}

pub(super) fn parse_source_attribute(raw: &str) -> Result<(&str, &str), String> {
    let Some(eq) = raw.find('=') else {
        return Err(format!(
            "--source-attribute \"{raw}\" is malformed (expected key=value)"
        ));
    };
    let key = &raw[..eq];
    if key.is_empty() {
        return Err(format!("--source-attribute \"{raw}\" has an empty key"));
    }
    let value = &raw[eq + 1..];
    Ok((key, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_with_attrs<'a>(project: &'a str, attrs: &'a [String]) -> PublishArgs<'a> {
        PublishArgs {
            type_id: "ai.task.completed.v1",
            data: "{}",
            project,
            source_name: None,
            source_attributes: attrs,
            parent: None,
            trace: None,
            dedupe_key: None,
            actor: crate::actor::ActorFlags::default(),
            quiet: false,
        }
    }

    #[test]
    fn build_metadata_repeated_attribute_last_wins_on_duplicate_key() {
        let attrs = ["foo=first".to_string(), "foo=second".to_string()];
        let args = args_with_attrs("demo", &attrs);
        let meta = build_metadata(&args, None).expect("valid");
        assert_eq!(meta.attributes.get("foo"), Some(&"second"));
    }

    #[test]
    fn build_metadata_attribute_without_equals_is_usage_error() {
        let attrs = ["bareword".to_string()];
        let args = args_with_attrs("demo", &attrs);
        let err = build_metadata(&args, None).expect_err("expected usage error");
        assert!(err.contains("bareword"), "got {err:?}");
        assert!(err.contains("--source-attribute"), "got {err:?}");
    }

    #[test]
    fn build_metadata_attribute_with_empty_key_is_usage_error() {
        let attrs = ["=value".to_string()];
        let args = args_with_attrs("demo", &attrs);
        let err = build_metadata(&args, None).expect_err("expected usage error");
        assert!(err.contains("empty key"), "got {err:?}");
    }

    #[test]
    fn build_metadata_attribute_with_empty_value_is_accepted() {
        // Empty value is fine; empty key is the only thing rejected.
        // Pin behaviour so a future "strict mode" doesn't silently
        // drift away from the wrapper-shared contract.
        let attrs = ["foo=".to_string()];
        let args = args_with_attrs("demo", &attrs);
        let meta = build_metadata(&args, None).expect("empty value must be accepted");
        assert_eq!(meta.attributes.get("foo"), Some(&""));
    }

    // ─── machine_hash injection ordering (Task 18) ───────────────────────

    #[test]
    fn build_metadata_machine_hash_inserts_under_machine_key_when_present() {
        let attrs: [String; 0] = [];
        let args = args_with_attrs("demo", &attrs);
        let meta = build_metadata(&args, Some("abcd1234ef567890")).expect("machine hash injection");
        assert_eq!(
            meta.attributes.get("machine"),
            Some(&"abcd1234ef567890"),
            "machine hash must land under the `machine` attributes key",
        );
        assert_eq!(
            meta.attributes.get("project"),
            Some(&"demo"),
            "project entry must remain alongside the machine hash",
        );
    }

    #[test]
    fn build_metadata_omits_machine_key_when_no_hash_provided() {
        let attrs: [String; 0] = [];
        let args = args_with_attrs("demo", &attrs);
        let meta = build_metadata(&args, None).expect("no machine hash");
        assert!(
            !meta.attributes.contains_key("machine"),
            "no `machine` key when hash is None; got {:?}",
            meta.attributes,
        );
    }

    #[test]
    fn build_metadata_source_attribute_machine_overrides_auto_hash_unit_pin() {
        // Direct unit-level pin of the override invariant the
        // integration test `publish_source_attribute_machine_flag_
        // overrides_auto_hash` covers end-to-end. Catches a regression
        // that swaps the insert order (auto-hash AFTER flag-loop)
        // without requiring a binary build + wiremock round-trip.
        let attrs = ["machine=manual-override".to_string()];
        let args = args_with_attrs("demo", &attrs);
        let meta =
            build_metadata(&args, Some("auto-computed-hash")).expect("override case must parse");
        assert_eq!(
            meta.attributes.get("machine"),
            Some(&"manual-override"),
            "--source-attribute machine= MUST overwrite the auto-computed hash",
        );
    }

    #[test]
    fn build_metadata_unrelated_flag_attributes_coexist_with_machine_hash() {
        // Mixed-key pin: auto-hash and unrelated flag attributes
        // (different keys) must both land. A mutation that overrides
        // ALL keys with the auto-hash (or inserts the auto-hash
        // somewhere wrong) would fail this test.
        let attrs = ["foo=bar".to_string(), "baz=qux".to_string()];
        let args = args_with_attrs("demo", &attrs);
        let meta =
            build_metadata(&args, Some("hash-value-here")).expect("mixed-key case must parse");
        assert_eq!(meta.attributes.get("machine"), Some(&"hash-value-here"));
        assert_eq!(meta.attributes.get("foo"), Some(&"bar"));
        assert_eq!(meta.attributes.get("baz"), Some(&"qux"));
        assert_eq!(meta.attributes.get("project"), Some(&"demo"));
    }
}
