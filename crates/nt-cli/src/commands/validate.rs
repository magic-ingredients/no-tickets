//! `nt validate <type> <data>` — local schema validation, no auth, no
//! network, no project resolution. Wraps `nt_schemas::validate` and
//! maps its `Option<Vec<ValidationIssue>>` to the structured-error
//! contract (Task 26).

use nt_schemas::validate;
use serde_json::Value;

use crate::error::NtError;

pub fn run(type_id: &str, data: &str) -> Result<(), NtError> {
    let parsed: Value = serde_json::from_str(data).map_err(|e| NtError::Usage {
        message: format!("--data must be valid JSON: {e}"),
    })?;

    match validate(type_id, &parsed) {
        None => Err(NtError::UnknownEventType {
            type_id: type_id.to_string(),
            suggestions: Vec::new(),
        }),
        Some(issues) if issues.is_empty() => {
            println!(r#"{{"valid":true}}"#);
            Ok(())
        }
        Some(issues) => Err(NtError::Validation {
            type_id: type_id.to_string(),
            batch_index: None,
            issues,
        }),
    }
}
