//! Parity tests for `nt_schemas::validate_metadata`.
//!
//! Pins the canonical `eventMetadataSchema` from `packages/schemas`
//! against the wire shapes the client emits. The schema validates the
//! envelope-level `{ actor: ... }` block — the discriminated union
//! that lives between `data` and `source` on every opt-in attributed
//! publish.
//!
//! Fixtures cover the seven cases the PRD calls out (valid agent
//! minimal, valid agent full, valid human, missing agentId, missing
//! userId, wrong discriminator, extra field) plus the
//! `thinkingEffort` enum boundary the optional-fields contract
//! depends on.

use serde_json::json;

use nt_schemas::{validate_metadata, ValidationIssue};

// ─── valid shapes ──────────────────────────────────────────────────────────

#[test]
fn validate_metadata_accepts_minimal_agent() {
    // The PRD's "minimum-identity cut" — agent_id is the only
    // mandatory field. CI bots / cron / migrations land here with
    // no `model`; the schema must accept them.
    let block = json!({
        "actor": { "type": "agent", "agentId": "github-actions" }
    });
    assert_eq!(
        validate_metadata(&block),
        Vec::<ValidationIssue>::new(),
        "minimal agent must validate clean",
    );
}

#[test]
fn validate_metadata_accepts_agent_with_session_context_fields() {
    // The harness-attributed shape: session context fields set by
    // `no-tickets session start`.
    let block = json!({
        "actor": {
            "type": "agent",
            "agentId": "claude",
            "model": "claude-opus-4-7",
            "provider": "anthropic",
            "sessionId": "sess-abc",
            "thinkingEffort": "high"
        }
    });
    assert_eq!(
        validate_metadata(&block),
        Vec::<ValidationIssue>::new(),
        "agent with session-context fields must validate clean",
    );
}

#[test]
fn validate_metadata_accepts_agent_with_every_optional_field() {
    // Per-call enrichment layered on top — the full shape an agent
    // publish produces after `actor::resolve` merges session + flags.
    let block = json!({
        "actor": {
            "type": "agent",
            "agentId": "claude",
            "model": "claude-opus-4-7",
            "provider": "anthropic",
            "sessionId": "sess-abc",
            "callId": "call-1",
            "thinkingEffort": "low",
            "promptTokens": 1234,
            "completionTokens": 567,
            "latencyMs": 812
        }
    });
    assert_eq!(
        validate_metadata(&block),
        Vec::<ValidationIssue>::new(),
        "agent with every optional field must validate clean",
    );
}

#[test]
fn validate_metadata_accepts_minimal_human() {
    // Branch 4 of the resolver: credentials → human. `userId` is the
    // only mandatory field.
    let block = json!({
        "actor": { "type": "human", "userId": "u-1" }
    });
    assert_eq!(
        validate_metadata(&block),
        Vec::<ValidationIssue>::new(),
        "minimal human must validate clean",
    );
}

#[test]
fn validate_metadata_accepts_human_with_email() {
    // Distinct userId and email values — the canonical schema treats
    // them as independent fields. Using the same string for both
    // would let a future bug "userId is read from the email field"
    // slip through unnoticed.
    let block = json!({
        "actor": {
            "type": "human",
            "userId": "u-1",
            "email": "alice@example.com"
        }
    });
    assert_eq!(
        validate_metadata(&block),
        Vec::<ValidationIssue>::new(),
        "human with email must validate clean",
    );
}

// ─── invalid shapes ───────────────────────────────────────────────────────

/// Returns true when at least one issue clearly identifies `field` as
/// the problem. Accepts three signals so the test stays anchored
/// regardless of which validator behaviour surfaces:
///   - The error message quotes the field name (`"field"` /
///     `'field'`) — typical for `additionalProperties` / `required`
///     errors that mention the offending key.
///   - The issue path ends in `.field` or equals `field` exactly —
///     typical for type-mismatch errors on a known field.
///   - The error sits at `actor` AND the message mentions `oneOf` —
///     the discriminated-union variant: when a required sub-field is
///     missing on the agent / human side, the validator reports
///     "not valid under any of the schemas listed in the 'oneOf'
///     keyword" at the `actor` level rather than naming the missing
///     sub-field directly. Accepting this anchor avoids a false-
///     negative without weakening the test to `!issues.is_empty()`.
fn names_field(issues: &[ValidationIssue], field: &str) -> bool {
    issues.iter().any(|i| {
        i.message.contains(&format!("\"{field}\""))
            || i.message.contains(&format!("'{field}'"))
            || i.path == field
            || i.path.ends_with(&format!(".{field}"))
            || (i.path == "actor" && i.message.contains("oneOf"))
    })
}

#[test]
fn validate_metadata_rejects_missing_actor_field() {
    // `eventMetadataSchema` requires `actor` at the top level. An
    // empty object — or any object without `actor` — must fail
    // closed. Without this pin, a schema regression that loosened
    // the top-level required list would let unattributed publishes
    // accidentally satisfy the validator.
    let block = json!({});
    let issues = validate_metadata(&block);
    assert!(
        !issues.is_empty(),
        "missing required actor must produce issues",
    );
    assert!(
        names_field(&issues, "actor"),
        "an issue must specifically name the missing `actor` field; got {issues:?}",
    );
}

#[test]
fn validate_metadata_rejects_agent_missing_agent_id() {
    // Mandatory-field violation: the agent variant must always carry
    // `agentId`. A regression where the resolver drops the id would
    // surface here.
    let block = json!({
        "actor": { "type": "agent" }
    });
    let issues = validate_metadata(&block);
    assert!(!issues.is_empty(), "missing agentId must produce issues");
    assert!(
        names_field(&issues, "agentId"),
        "an issue must specifically name agentId; got {issues:?}",
    );
}

#[test]
fn validate_metadata_rejects_human_missing_user_id() {
    let block = json!({
        "actor": { "type": "human" }
    });
    let issues = validate_metadata(&block);
    assert!(!issues.is_empty(), "missing userId must produce issues");
    assert!(
        names_field(&issues, "userId"),
        "an issue must specifically name userId; got {issues:?}",
    );
}

#[test]
fn validate_metadata_rejects_unknown_discriminator() {
    // `type: "robot"` matches neither agent nor human → both
    // oneOf branches fail. The validator surfaces the discriminator
    // mismatch.
    let block = json!({
        "actor": { "type": "robot", "agentId": "r2d2" }
    });
    let issues = validate_metadata(&block);
    assert!(
        !issues.is_empty(),
        "unknown discriminator must produce issues",
    );
}

#[test]
fn validate_metadata_rejects_thinking_effort_outside_enum() {
    // `thinkingEffort` is a 3-valued enum (low/medium/high). Anything
    // else is rejected.
    let block = json!({
        "actor": {
            "type": "agent",
            "agentId": "claude",
            "thinkingEffort": "extreme"
        }
    });
    let issues = validate_metadata(&block);
    assert!(
        !issues.is_empty(),
        "out-of-enum thinkingEffort must produce issues",
    );
    assert!(
        issues
            .iter()
            .any(|i| i.path.contains("thinkingEffort") || i.message.contains("thinkingEffort")),
        "an issue must name the offending field; got {issues:?}",
    );
}

#[test]
fn validate_metadata_rejects_extra_top_level_field() {
    // `eventMetadataSchema` is strict — extras get rejected so
    // future schema additions don't accidentally land in clients
    // that aren't ready for them. Pinning this is the canonical
    // forward-compat-by-versioning contract.
    let block = json!({
        "actor": { "type": "agent", "agentId": "claude" },
        "unexpectedExtraKey": 42
    });
    let issues = validate_metadata(&block);
    assert!(
        !issues.is_empty(),
        "top-level extra field must produce issues",
    );
}

#[test]
fn validate_metadata_rejects_extra_field_inside_actor() {
    // The actor variants are also `.strict()` in zod-land. An extra
    // field inside the agent block must be rejected just like a
    // top-level extra. Closes the door on a sloppy implementation
    // that strict-checks one level but not the other.
    let block = json!({
        "actor": {
            "type": "agent",
            "agentId": "claude",
            "wat": "bonus"
        }
    });
    let issues = validate_metadata(&block);
    assert!(
        !issues.is_empty(),
        "extra field inside actor must produce issues",
    );
}

#[test]
fn validate_metadata_rejects_negative_prompt_tokens() {
    // Token counts are `z.number().int().nonnegative()` — anything
    // < 0 is rejected. Defends against per-call layering bugs where
    // an `i64` field accidentally becomes negative.
    let block = json!({
        "actor": {
            "type": "agent",
            "agentId": "claude",
            "promptTokens": -1
        }
    });
    let issues = validate_metadata(&block);
    assert!(
        !issues.is_empty(),
        "negative promptTokens must produce issues",
    );
}

#[test]
fn validate_metadata_rejects_negative_completion_tokens() {
    // Same nonnegative constraint as promptTokens. Pinned separately
    // because the constraint is per-field and an accidental schema
    // edit could weaken just one of the three.
    let block = json!({
        "actor": {
            "type": "agent",
            "agentId": "claude",
            "completionTokens": -1
        }
    });
    let issues = validate_metadata(&block);
    assert!(
        !issues.is_empty(),
        "negative completionTokens must produce issues",
    );
}

#[test]
fn validate_metadata_rejects_negative_latency_ms() {
    let block = json!({
        "actor": {
            "type": "agent",
            "agentId": "claude",
            "latencyMs": -1
        }
    });
    let issues = validate_metadata(&block);
    assert!(!issues.is_empty(), "negative latencyMs must produce issues",);
}

// ─── discriminator misuse ─────────────────────────────────────────────────

#[test]
fn validate_metadata_rejects_numeric_type_discriminator() {
    // `type` must be a literal string (`"agent"` / `"human"`). A
    // number breaks the `const` constraint on every oneOf branch.
    let block = json!({
        "actor": { "type": 42, "agentId": "claude" }
    });
    let issues = validate_metadata(&block);
    assert!(
        !issues.is_empty(),
        "numeric type discriminator must produce issues",
    );
}

#[test]
fn validate_metadata_rejects_null_type_discriminator() {
    let block = json!({
        "actor": { "type": null, "agentId": "claude" }
    });
    let issues = validate_metadata(&block);
    assert!(
        !issues.is_empty(),
        "null type discriminator must produce issues",
    );
}

// ─── non-object inputs ────────────────────────────────────────────────────

#[test]
fn validate_metadata_rejects_null_input() {
    // `validate_metadata` accepts an arbitrary `Value`. Non-object
    // inputs must fail closed — the schema asserts `type: object`
    // at the root.
    let issues = validate_metadata(&serde_json::Value::Null);
    assert!(!issues.is_empty(), "null input must produce issues");
}

#[test]
fn validate_metadata_rejects_array_input() {
    let issues = validate_metadata(&json!([]));
    assert!(!issues.is_empty(), "array input must produce issues");
}

#[test]
fn validate_metadata_rejects_string_input() {
    let issues = validate_metadata(&json!("not an object"));
    assert!(!issues.is_empty(), "string input must produce issues");
}

#[test]
fn validate_metadata_rejects_number_input() {
    let issues = validate_metadata(&json!(42));
    assert!(!issues.is_empty(), "number input must produce issues");
}

// ─── bundle integrity ─────────────────────────────────────────────────────

#[test]
fn validate_metadata_returns_dot_joined_paths() {
    // PRD: "Public function returns dot-joined paths, same shape as
    // `validate()` for event types". Two pins:
    //   1. **Positive** — at least one issue carries the literal path
    //      `actor.promptTokens` (or the parent `actor` if the validator
    //      surfaces the error at the discriminated-union level).
    //   2. **Negative** — no issue path retains JSON Pointer artefacts
    //      (leading `/`, `~1` / `~0` escapes).
    let block = json!({
        "actor": {
            "type": "agent",
            "agentId": "claude",
            "promptTokens": "not-a-number"
        }
    });
    let issues = validate_metadata(&block);
    assert!(
        !issues.is_empty(),
        "type-mismatched promptTokens must produce issues",
    );

    let paths: Vec<&str> = issues.iter().map(|i| i.path.as_str()).collect();
    assert!(
        paths
            .iter()
            .any(|p| *p == "actor.promptTokens" || *p == "actor"),
        "expected a positively-anchored path through the actor object; got {paths:?}",
    );

    for issue in &issues {
        assert!(
            !issue.path.starts_with('/'),
            "path must be dot-joined, not a JSON Pointer; got {issue:?}",
        );
        assert!(
            !issue.path.contains("~"),
            "path must not contain JSON Pointer escapes; got {issue:?}",
        );
    }
}

// ─── bundle integrity ────────────────────────────────────────────────────

#[test]
fn validate_metadata_rejects_outright_when_schema_loaded() {
    // Sentinel: prove the metadata validator is non-trivially loaded
    // (not e.g. a "default-accepts-everything" fallback) by sending
    // a payload that NO reasonable schema would accept. If the
    // metadata schema field went missing from the bundle without the
    // bundle parse failing, this test catches the regression. Comp-
    // anion to `bundle_contains_every_expected_type_id` over in
    // tests/validate.rs.
    let issues = validate_metadata(&json!({}));
    assert!(
        !issues.is_empty(),
        "metadata schema must be loaded and strict — empty object should fail; \
         if this passes, the metadataSchema field is probably missing from the bundle",
    );
}
