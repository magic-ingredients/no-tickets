//! Local JSON Schema validation against the bundled event-type schemas.
//!
//! Mirrors the TS reference at `src/cli/lib/schema-validate.ts`:
//! `validateEventLocally(typeId, data) → ValidationIssue[]`. The Rust
//! port returns `Option<Vec<ValidationIssue>>`:
//!   - `None`              — unknown type id (TS guards via `isKnownEventType`)
//!   - `Some(Vec::new())`  — valid
//!   - `Some(issues)`      — invalid; path is dot-joined for TS parity
//!
//! Bundle source: `build.rs` fetches the `schemas-v<VERSION>.json.gz`
//! asset from the no-tickets-service GH Release, verifies the
//! published sha256 sidecar, decompresses, and writes the JSON to
//! `$OUT_DIR/event-types.bundle.json`. The pinned `SCHEMAS_VERSION`
//! constant in `build.rs` is the single bump-point for tracking a new
//! schemas release. The validator API surface here is independent of
//! how the bundle was obtained.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use jsonschema::Validator;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationIssue {
    pub path: String,
    pub message: String,
}

const BUNDLE_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/event-types.bundle.json"));

#[derive(Debug, Deserialize)]
struct BundleFile {
    /// Self-reported schemas-package version baked into the bundle at
    /// release time. Matches the upstream package.json `version`. The
    /// `bundle_version_matches_pinned_metadata` test asserts this lines
    /// up with the version pinned in `build.rs`.
    version: String,
    // BTreeMap so `into_iter()` yields entries in deterministic sorted
    // key order without an extra sort step. `known_type_ids` returns
    // those keys verbatim — both the test ordering pin and TS object-
    // literal stability ride on this.
    schemas: BTreeMap<String, Value>,
    /// Envelope-level `metadata` block schema — singleton, shared by
    /// every event type. Added to the bundle at schemas-v0.2.2.
    /// Consumed by `validate_metadata`.
    #[serde(rename = "metadataSchema")]
    metadata_schema: Value,
}

struct CompiledBundle {
    version: String,
    /// Sorted vec of (type_id, compiled_validator). Vec rather than
    /// HashMap because n=11 and `known_type_ids` needs a stable
    /// iteration order anyway.
    entries: Vec<(String, Validator)>,
    metadata_validator: Validator,
}

fn bundle() -> &'static CompiledBundle {
    static CELL: OnceLock<CompiledBundle> = OnceLock::new();
    CELL.get_or_init(|| {
        let parsed: BundleFile = serde_json::from_str(BUNDLE_JSON).expect("bundle JSON parses");
        let entries: Vec<(String, Validator)> = parsed
            .schemas
            .into_iter()
            .map(|(type_id, schema)| {
                // `should_validate_formats(true)` makes the validator
                // enforce `format` keywords (date-time, email, etc.)
                // as assertions instead of treating them as
                // annotation-only per Draft 2020-12's default. The
                // bundled schemas use `format: "date-time"`; without
                // this, format violations would silently pass.
                let validator = jsonschema::draft202012::options()
                    .should_validate_formats(true)
                    .build(&schema)
                    .unwrap_or_else(|e| panic!("schema for {type_id:?} failed to compile: {e}"));
                (type_id, validator)
            })
            .collect();
        let metadata_validator = jsonschema::draft202012::options()
            .should_validate_formats(true)
            .build(&parsed.metadata_schema)
            .unwrap_or_else(|e| panic!("metadataSchema failed to compile: {e}"));
        CompiledBundle {
            version: parsed.version,
            entries,
            metadata_validator,
        }
    })
}

/// Version of the upstream `@magic-ingredients/no-tickets-schemas`
/// package the bundle was generated from. Read from the bundle's own
/// `version` field; resolved on first call to any nt-schemas API (the
/// OnceLock initialiser parses the bundle and compiles every schema,
/// which is where any bundle-integrity bug would panic).
pub fn bundle_version() -> &'static str {
    bundle().version.as_str()
}

/// Sorted list of every event-type id the bundle knows about.
pub fn known_type_ids() -> Vec<&'static str> {
    bundle().entries.iter().map(|(k, _)| k.as_str()).collect()
}

/// Validate `data` against the bundled schema for `type_id`.
///
/// - `None` — `type_id` is not in the bundle (caller should surface
///   `unknown_event_type`).
/// - `Some(Vec::new())` — data validates cleanly.
/// - `Some(issues)` — data fails validation; each issue carries a
///   dot-joined `path` (matching TS `validateEventLocally`'s shape)
///   and a human-readable `message`.
pub fn validate(type_id: &str, data: &Value) -> Option<Vec<ValidationIssue>> {
    let validator = bundle()
        .entries
        .iter()
        .find_map(|(k, v)| (k == type_id).then_some(v))?;

    let issues: Vec<ValidationIssue> = validator
        .iter_errors(data)
        .map(|err| ValidationIssue {
            path: json_pointer_to_dot_path(&err.instance_path().to_string()),
            message: err.to_string(),
        })
        .collect();
    Some(issues)
}

/// Validate the envelope-level `metadata` block against the canonical
/// `eventMetadataSchema` from `@magic-ingredients/no-tickets-schemas`.
///
/// The argument is the full metadata object (`{ "actor": {...} }`), not
/// the actor block in isolation — the schema is strict at both the
/// envelope level and the actor variants, so validating the wrapper
/// catches extras at either level.
///
/// Returns a (possibly empty) `Vec<ValidationIssue>`. No `Option`
/// wrapper — the metadata schema is a singleton in the bundle, always
/// present in a v0.2.2+ release.
pub fn validate_metadata(metadata: &Value) -> Vec<ValidationIssue> {
    bundle()
        .metadata_validator
        .iter_errors(metadata)
        .map(|err| ValidationIssue {
            path: json_pointer_to_dot_path(&err.instance_path().to_string()),
            message: err.to_string(),
        })
        .collect()
}

/// Convert a JSON Pointer (`/foo/bar`, `/items/0/name`) to the
/// dot-joined path style TS uses (`foo.bar`, `items.0.name`). Empty
/// pointer (i.e. error at the document root) becomes the empty string.
fn json_pointer_to_dot_path(pointer: &str) -> String {
    let trimmed = pointer.strip_prefix('/').unwrap_or(pointer);
    if trimmed.is_empty() {
        return String::new();
    }
    // RFC 6901 escapes `/` as `~1` and `~` as `~0` inside path segments.
    // Field names in this codebase don't include either character, but
    // unescape defensively so a future schema change doesn't surprise us.
    trimmed
        .split('/')
        .map(|seg| seg.replace("~1", "/").replace("~0", "~"))
        .collect::<Vec<_>>()
        .join(".")
}

#[cfg(test)]
mod internal_tests {
    use super::*;

    #[test]
    fn json_pointer_root_to_empty_string() {
        assert_eq!(json_pointer_to_dot_path(""), "");
        assert_eq!(json_pointer_to_dot_path("/"), "");
    }

    #[test]
    fn json_pointer_single_segment() {
        assert_eq!(json_pointer_to_dot_path("/taskId"), "taskId");
    }

    #[test]
    fn json_pointer_nested_segments() {
        assert_eq!(json_pointer_to_dot_path("/outer/inner"), "outer.inner");
    }

    #[test]
    fn json_pointer_array_index_segment() {
        assert_eq!(json_pointer_to_dot_path("/items/0/name"), "items.0.name");
    }
}
