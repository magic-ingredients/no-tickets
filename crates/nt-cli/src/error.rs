//! `NtError` — the public structured-error contract for the `nt` binary.
//!
//! Per `docs/binary-error-contract.md`, every command failure path maps
//! to one of these variants. The variant determines:
//!
//! - the process exit code (`exit_code()`) — 1–7 today, 64+ reserved
//! - the JSON shape on stderr when stderr is a pipe (`to_json()`)
//! - the human-readable line on stderr when stderr is a TTY (`to_human()`)
//!
//! The wire contract is **additive-only** across binary releases:
//! wrappers compiled against an old binary must continue to function
//! against new ones. Practically that means new variants get new exit
//! codes (≥ 8) and new fields get added, but existing variant names,
//! exit codes, and field names never change or disappear.

// RED phase: the methods below are exercised only by in-file unit
// tests; the public surface isn't yet wired through `main.rs` →
// command-level migration. GREEN-phase code drops this allow when
// `commands::publish` / `commands::validate` start returning
// `Result<(), NtError>` and main.rs calls `emit_and_exit_code`.
#![allow(dead_code, unused_variables)]

use nt_schemas::ValidationIssue;
use thiserror::Error;

/// Typed failure modes for `nt` commands.
///
/// Each variant carries the structured payload that `to_json()`
/// serialises into the documented stderr shape. The `#[error(...)]`
/// strings drive the `Display` impl (used as the fallback when
/// `to_human()` callers want a one-liner without context).
#[derive(Debug, Error)]
pub enum NtError {
    /// Local schema validation failure. `batch_index` is `Some(i)` when
    /// the failure was on line `i` of a JSONL batch (Task 16) and
    /// `None` for single-event mode.
    #[error("{} validation issue(s) for {type_id}", issues.len())]
    Validation {
        type_id: String,
        batch_index: Option<usize>,
        issues: Vec<ValidationIssue>,
    },
    /// Type id not present in the local registry. `suggestions` carries
    /// fuzzy-match candidates when available (the CLI populates them
    /// via the same fuzzy matcher the publish command uses today).
    #[error("unknown event type: {type_id}")]
    UnknownEventType {
        type_id: String,
        suggestions: Vec<String>,
    },
    /// Server returned 403. `domain` is the resource domain rejected
    /// (e.g. `events`, `tokens`) so wrappers can offer a targeted hint.
    #[error("permission denied for {domain}")]
    PermissionDenied { domain: String },
    /// Network or 5xx after the configured retry budget. `retriable`
    /// signals whether the caller should expect this class to clear on
    /// its own — true for 5xx/connection-reset, false for 4xx other
    /// than 401/403 (which map to NotAuthenticated/PermissionDenied).
    #[error("transport error: {message}")]
    Transport { message: String, retriable: bool },
    /// Bearer token absent, mis-shaped, or rejected (server 401).
    #[error("not authenticated: {message}")]
    NotAuthenticated { message: String },
    /// `--project <name>` referenced a project that's not in the local
    /// config registry. `known_projects` carries the locally-registered
    /// project names so wrappers can prompt or auto-complete.
    #[error("project not registered: {project}")]
    ProjectNotRegistered {
        project: String,
        known_projects: Vec<String>,
    },
    /// Malformed flags / missing required arguments / mutually-exclusive
    /// flag combinations. Covers everything that's the *caller's* fault
    /// rather than the server's or the schema's.
    #[error("{message}")]
    Usage { message: String },
}

impl NtError {
    /// Documented exit code per `docs/binary-error-contract.md`.
    pub fn exit_code(&self) -> i32 {
        unimplemented!("exit_code: GREEN phase pending")
    }

    /// Stable `"error": "<class>"` discriminator. Pinned by tests so a
    /// future rename can't silently break wrapper parsers.
    pub fn class(&self) -> &'static str {
        unimplemented!("class: GREEN phase pending")
    }

    /// JSON object for stderr-on-pipe. The shape per variant is
    /// documented in the binary error contract; tests pin every field.
    pub fn to_json(&self) -> serde_json::Value {
        unimplemented!("to_json: GREEN phase pending")
    }

    /// Human-readable line for stderr-on-TTY. No JSON braces, no field
    /// names — just the most useful single message the user can act on.
    pub fn to_human(&self) -> String {
        unimplemented!("to_human: GREEN phase pending")
    }
}

/// Pure formatter — JSON one-liner when stderr is a pipe (machine-
/// readable for wrappers), human one-liner when stderr is a TTY
/// (readable for interactive use).
///
/// Pure on purpose: keeps the test surface small, avoids the production
/// TTY-detection path in test mode, and matches the spec's
/// additive-only contract by serialising via `to_json` / `to_human`.
pub fn format_for(is_tty: bool, err: &NtError) -> String {
    unimplemented!("format_for: GREEN phase pending")
}

/// Map a `Result<(), NtError>` from a command's `run()` to a process
/// exit code, emitting the error (if any) to `stderr_writer` first.
/// `is_tty` controls format; production wires
/// `std::io::stderr().is_terminal()`.
pub fn emit_and_exit_code<W: std::io::Write>(
    result: Result<(), NtError>,
    stderr_writer: &mut W,
    is_tty: bool,
) -> i32 {
    unimplemented!("emit_and_exit_code: GREEN phase pending")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(path: &str, message: &str) -> ValidationIssue {
        ValidationIssue {
            path: path.to_string(),
            message: message.to_string(),
        }
    }

    // ---- exit_code -----------------------------------------------------

    #[test]
    fn exit_code_validation_is_one() {
        let err = NtError::Validation {
            type_id: "a.b.c.v1".into(),
            batch_index: None,
            issues: vec![issue("/x", "required")],
        };
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn exit_code_unknown_event_type_is_two() {
        let err = NtError::UnknownEventType {
            type_id: "no.such.type.v1".into(),
            suggestions: vec![],
        };
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn exit_code_permission_denied_is_three() {
        let err = NtError::PermissionDenied {
            domain: "events".into(),
        };
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn exit_code_transport_is_four() {
        let err = NtError::Transport {
            message: "server returned 503".into(),
            retriable: true,
        };
        assert_eq!(err.exit_code(), 4);
    }

    #[test]
    fn exit_code_not_authenticated_is_five() {
        let err = NtError::NotAuthenticated {
            message: "no credentials".into(),
        };
        assert_eq!(err.exit_code(), 5);
    }

    #[test]
    fn exit_code_project_not_registered_is_six() {
        let err = NtError::ProjectNotRegistered {
            project: "demo".into(),
            known_projects: vec!["prod".into()],
        };
        assert_eq!(err.exit_code(), 6);
    }

    #[test]
    fn exit_code_usage_is_seven() {
        let err = NtError::Usage {
            message: "--data must be valid JSON".into(),
        };
        assert_eq!(err.exit_code(), 7);
    }

    #[test]
    fn exit_codes_are_all_distinct_and_in_documented_range() {
        // Defends against an accidental copy-paste duplicate when a new
        // variant lands. Every documented code (1–7) must appear
        // exactly once; nothing falls outside [1, 63] (64+ reserved).
        let all = [
            NtError::Validation {
                type_id: "x".into(),
                batch_index: None,
                issues: vec![],
            }
            .exit_code(),
            NtError::UnknownEventType {
                type_id: "x".into(),
                suggestions: vec![],
            }
            .exit_code(),
            NtError::PermissionDenied { domain: "x".into() }.exit_code(),
            NtError::Transport {
                message: "x".into(),
                retriable: false,
            }
            .exit_code(),
            NtError::NotAuthenticated {
                message: "x".into(),
            }
            .exit_code(),
            NtError::ProjectNotRegistered {
                project: "x".into(),
                known_projects: vec![],
            }
            .exit_code(),
            NtError::Usage {
                message: "x".into(),
            }
            .exit_code(),
        ];
        let mut seen = Vec::new();
        for code in all {
            assert!(
                (1..64).contains(&code),
                "exit code {code} out of documented range [1, 63]"
            );
            assert!(!seen.contains(&code), "duplicate exit code: {code}");
            seen.push(code);
        }
        assert_eq!(seen.len(), 7, "all 7 documented variants must be covered");
    }

    // ---- class() discriminator -----------------------------------------

    #[test]
    fn class_strings_match_documented_contract() {
        // The `"error": "<class>"` field is the wrapper's primary
        // discriminator. Pin each value so a future rename surfaces as
        // a test failure rather than a silent wire-format change.
        assert_eq!(
            NtError::Validation {
                type_id: "x".into(),
                batch_index: None,
                issues: vec![]
            }
            .class(),
            "validation_error"
        );
        assert_eq!(
            NtError::UnknownEventType {
                type_id: "x".into(),
                suggestions: vec![]
            }
            .class(),
            "unknown_event_type"
        );
        assert_eq!(
            NtError::PermissionDenied { domain: "x".into() }.class(),
            "permission_denied"
        );
        assert_eq!(
            NtError::Transport {
                message: "x".into(),
                retriable: false
            }
            .class(),
            "transport_error"
        );
        assert_eq!(
            NtError::NotAuthenticated {
                message: "x".into()
            }
            .class(),
            "not_authenticated"
        );
        assert_eq!(
            NtError::ProjectNotRegistered {
                project: "x".into(),
                known_projects: vec![]
            }
            .class(),
            "project_not_registered"
        );
        assert_eq!(
            NtError::Usage {
                message: "x".into()
            }
            .class(),
            "usage"
        );
    }

    // ---- to_json: per-variant shape -----------------------------------

    #[test]
    fn json_validation_includes_type_id_issues_and_omits_batch_when_none() {
        let err = NtError::Validation {
            type_id: "ai.task.completed.v1".into(),
            batch_index: None,
            issues: vec![
                issue("/taskId", "is required"),
                issue("/sessionId", "is required"),
            ],
        };
        let v = err.to_json();
        assert_eq!(v["error"], "validation_error");
        assert_eq!(v["typeId"], "ai.task.completed.v1");
        assert!(
            v.get("batchIndex").is_none() || v["batchIndex"].is_null(),
            "batchIndex must be absent or null in single-event mode, got: {v:?}"
        );
        let issues = v["issues"].as_array().expect("issues array");
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0]["path"], "/taskId");
        assert_eq!(issues[0]["message"], "is required");
        assert_eq!(issues[1]["path"], "/sessionId");
    }

    #[test]
    fn json_validation_includes_batch_index_when_some() {
        let err = NtError::Validation {
            type_id: "ai.task.completed.v1".into(),
            batch_index: Some(3),
            issues: vec![issue("/taskId", "is required")],
        };
        let v = err.to_json();
        assert_eq!(v["batchIndex"], 3);
    }

    #[test]
    fn json_unknown_event_type_includes_type_id_and_suggestions() {
        let err = NtError::UnknownEventType {
            type_id: "ai.taks.completed.v1".into(),
            suggestions: vec!["ai.task.completed.v1".into(), "ai.task.created.v1".into()],
        };
        let v = err.to_json();
        assert_eq!(v["error"], "unknown_event_type");
        assert_eq!(v["typeId"], "ai.taks.completed.v1");
        let suggestions = v["suggestions"].as_array().expect("suggestions array");
        assert_eq!(suggestions.len(), 2);
        assert_eq!(suggestions[0], "ai.task.completed.v1");
    }

    #[test]
    fn json_unknown_event_type_keeps_empty_suggestions_array_not_null() {
        // Wrappers iterate `suggestions` unconditionally; null vs []
        // would force every wrapper into a defensive cast. Pin: empty.
        let err = NtError::UnknownEventType {
            type_id: "x".into(),
            suggestions: vec![],
        };
        let v = err.to_json();
        assert!(
            v["suggestions"].is_array() && v["suggestions"].as_array().unwrap().is_empty(),
            "suggestions must be [] not null, got: {v:?}"
        );
    }

    #[test]
    fn json_permission_denied_includes_domain() {
        let err = NtError::PermissionDenied {
            domain: "events".into(),
        };
        let v = err.to_json();
        assert_eq!(v["error"], "permission_denied");
        assert_eq!(v["domain"], "events");
    }

    #[test]
    fn json_transport_includes_message_and_retriable() {
        let err = NtError::Transport {
            message: "server returned 503".into(),
            retriable: true,
        };
        let v = err.to_json();
        assert_eq!(v["error"], "transport_error");
        assert_eq!(v["message"], "server returned 503");
        assert_eq!(v["retriable"], true);
    }

    #[test]
    fn json_transport_retriable_false_for_4xx() {
        let err = NtError::Transport {
            message: "server returned 422".into(),
            retriable: false,
        };
        let v = err.to_json();
        assert_eq!(v["retriable"], false);
    }

    #[test]
    fn json_not_authenticated_includes_message() {
        let err = NtError::NotAuthenticated {
            message: "no credentials configured".into(),
        };
        let v = err.to_json();
        assert_eq!(v["error"], "not_authenticated");
        assert_eq!(v["message"], "no credentials configured");
    }

    #[test]
    fn json_project_not_registered_includes_project_and_known_projects() {
        let err = NtError::ProjectNotRegistered {
            project: "missing".into(),
            known_projects: vec!["demo".into(), "prod".into()],
        };
        let v = err.to_json();
        assert_eq!(v["error"], "project_not_registered");
        assert_eq!(v["project"], "missing");
        let known = v["knownProjects"].as_array().expect("knownProjects array");
        assert_eq!(known.len(), 2);
        assert_eq!(known[0], "demo");
    }

    #[test]
    fn json_project_not_registered_keeps_empty_known_array_not_null() {
        let err = NtError::ProjectNotRegistered {
            project: "missing".into(),
            known_projects: vec![],
        };
        let v = err.to_json();
        assert!(
            v["knownProjects"].is_array() && v["knownProjects"].as_array().unwrap().is_empty(),
            "knownProjects must be [] not null, got: {v:?}"
        );
    }

    #[test]
    fn json_usage_includes_message() {
        let err = NtError::Usage {
            message: "--data must be valid JSON".into(),
        };
        let v = err.to_json();
        assert_eq!(v["error"], "usage");
        assert_eq!(v["message"], "--data must be valid JSON");
    }

    // ---- to_human ------------------------------------------------------

    #[test]
    fn human_validation_names_type_id_and_issue_count() {
        let err = NtError::Validation {
            type_id: "ai.task.completed.v1".into(),
            batch_index: None,
            issues: vec![
                issue("/taskId", "is required"),
                issue("/sessionId", "is required"),
            ],
        };
        let human = err.to_human();
        assert!(human.contains("ai.task.completed.v1"), "got: {human}");
        assert!(
            human.contains("2"),
            "human should name issue count, got: {human}"
        );
    }

    #[test]
    fn human_unknown_event_type_names_the_type_id() {
        let err = NtError::UnknownEventType {
            type_id: "no.such.type.v1".into(),
            suggestions: vec![],
        };
        let human = err.to_human();
        assert!(human.contains("no.such.type.v1"), "got: {human}");
    }

    #[test]
    fn human_unknown_event_type_lists_suggestions_when_present() {
        let err = NtError::UnknownEventType {
            type_id: "ai.taks.completed.v1".into(),
            suggestions: vec!["ai.task.completed.v1".into()],
        };
        let human = err.to_human();
        assert!(
            human.contains("ai.task.completed.v1"),
            "human should name the suggestion, got: {human}"
        );
    }

    #[test]
    fn human_project_not_registered_names_project() {
        let err = NtError::ProjectNotRegistered {
            project: "missing".into(),
            known_projects: vec!["demo".into()],
        };
        let human = err.to_human();
        assert!(human.contains("missing"), "got: {human}");
    }

    #[test]
    fn human_lines_never_contain_json_braces() {
        // The whole point of the human variant: no machine-readable
        // braces leak into a TTY render. Pin across every variant so a
        // careless refactor can't accidentally route to_json through
        // to_human.
        let variants = [
            NtError::Validation {
                type_id: "x".into(),
                batch_index: None,
                issues: vec![],
            },
            NtError::UnknownEventType {
                type_id: "x".into(),
                suggestions: vec![],
            },
            NtError::PermissionDenied { domain: "x".into() },
            NtError::Transport {
                message: "x".into(),
                retriable: true,
            },
            NtError::NotAuthenticated {
                message: "x".into(),
            },
            NtError::ProjectNotRegistered {
                project: "x".into(),
                known_projects: vec![],
            },
            NtError::Usage {
                message: "x".into(),
            },
        ];
        for err in &variants {
            let human = err.to_human();
            assert!(
                !human.contains('{') && !human.contains('}'),
                "human render leaked JSON braces for {err:?}: {human}"
            );
        }
    }

    // ---- format_for: TTY vs pipe routing -------------------------------

    #[test]
    fn format_for_pipe_returns_single_line_json() {
        let err = NtError::Usage {
            message: "missing --type".into(),
        };
        let out = format_for(false, &err);
        assert!(out.starts_with('{'), "pipe format must be JSON, got: {out}");
        assert!(out.ends_with('}'), "pipe format must be JSON, got: {out}");
        assert!(
            !out.contains('\n'),
            "pipe format must be single-line for line-based wrapper parsers, got: {out:?}"
        );
        // Must round-trip through serde_json.
        let parsed: serde_json::Value =
            serde_json::from_str(&out).expect("pipe format must be valid JSON");
        assert_eq!(parsed["error"], "usage");
    }

    #[test]
    fn format_for_tty_returns_human_readable() {
        let err = NtError::Usage {
            message: "missing --type".into(),
        };
        let out = format_for(true, &err);
        assert!(
            !out.starts_with('{'),
            "tty format must not be JSON, got: {out}"
        );
        assert!(out.contains("missing --type"), "got: {out}");
    }

    // ---- emit_and_exit_code -------------------------------------------

    #[test]
    fn emit_and_exit_code_ok_returns_zero_and_writes_nothing_to_stderr() {
        let mut stderr = Vec::new();
        let code = emit_and_exit_code(Ok(()), &mut stderr, false);
        assert_eq!(code, 0);
        assert!(
            stderr.is_empty(),
            "successful command must not write to stderr, got: {:?}",
            String::from_utf8_lossy(&stderr)
        );
    }

    #[test]
    fn emit_and_exit_code_err_returns_variant_exit_and_writes_json_to_pipe() {
        let mut stderr = Vec::new();
        let code = emit_and_exit_code(
            Err(NtError::PermissionDenied {
                domain: "events".into(),
            }),
            &mut stderr,
            false,
        );
        assert_eq!(code, 3);
        let s = String::from_utf8(stderr).unwrap();
        assert!(
            s.ends_with('\n'),
            "stderr line must be newline-terminated for line-based readers, got: {s:?}"
        );
        let parsed: serde_json::Value = serde_json::from_str(s.trim_end()).unwrap();
        assert_eq!(parsed["error"], "permission_denied");
        assert_eq!(parsed["domain"], "events");
    }

    #[test]
    fn emit_and_exit_code_err_writes_human_to_tty() {
        let mut stderr = Vec::new();
        let code = emit_and_exit_code(
            Err(NtError::Usage {
                message: "missing --type".into(),
            }),
            &mut stderr,
            true,
        );
        assert_eq!(code, 7);
        let s = String::from_utf8(stderr).unwrap();
        assert!(!s.contains('{'), "tty render must not be JSON, got: {s}");
        assert!(s.contains("missing --type"), "got: {s}");
    }
}
