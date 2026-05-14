//! `nt validate <type> <data>` — local schema validation, no auth, no
//! network, no project resolution. Wraps `nt_schemas::validate` and
//! maps its `Option<Vec<ValidationIssue>>` to the spike-scope exit-code
//! contract (Task 4a will define the full structured-error shape).

use nt_schemas::{validate, ValidationIssue};
use serde_json::Value;

const EXIT_OK: i32 = 0;
const EXIT_VALIDATION: i32 = 1;
const EXIT_UNKNOWN_TYPE: i32 = 2;

pub fn run(type_id: &str, data: &str) -> i32 {
    let parsed: Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("--data must be valid JSON: {e}");
            return EXIT_VALIDATION;
        }
    };

    match validate(type_id, &parsed) {
        None => {
            eprintln!("Unknown event type: {type_id}");
            EXIT_UNKNOWN_TYPE
        }
        Some(issues) if issues.is_empty() => {
            println!(r#"{{"valid":true}}"#);
            EXIT_OK
        }
        Some(issues) => {
            print_issues(type_id, &issues);
            EXIT_VALIDATION
        }
    }
}

fn print_issues(type_id: &str, issues: &[ValidationIssue]) {
    eprintln!(
        "{type_id}: {n} local validation error(s):",
        n = issues.len()
    );
    for issue in issues {
        eprintln!("  {}: {}", issue.path, issue.message);
    }
}
