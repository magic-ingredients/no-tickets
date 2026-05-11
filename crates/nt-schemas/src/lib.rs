//! Not yet implemented — stub for TDD RED phase.

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationIssue {
    pub path: String,
    pub message: String,
}

pub const BUNDLE_VERSION: &str = "STUB";

/// Returns:
/// - `None` if the type id is unknown (no schema in bundle).
/// - `Some(Vec::new())` if data validates cleanly.
/// - `Some(issues)` if data fails validation.
pub fn validate(_type_id: &str, _data: &Value) -> Option<Vec<ValidationIssue>> {
    panic!("nt-schemas::validate not yet implemented");
}

/// Sorted list of all event-type ids in the bundle.
pub fn known_type_ids() -> Vec<&'static str> {
    panic!("nt-schemas::known_type_ids not yet implemented");
}
