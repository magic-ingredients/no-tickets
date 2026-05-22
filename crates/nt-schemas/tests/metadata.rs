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
    // The shape `actor::resolve` produces from session credentials —
    // both userId and email present.
    let block = json!({
        "actor": {
            "type": "human",
            "userId": "alice@example.com",
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

#[test]
fn validate_metadata_rejects_agent_missing_agent_id() {
    // Mandatory-field violation: the agent variant must always carry
    // `agentId`. A regression where the resolver drops the id would
    // surface here.
    let block = json!({
        "actor": { "type": "agent" }
    });
    let issues = validate_metadata(&block);
    assert!(!issues.is_empty(), "missing agentId must produce issues",);
    assert!(
        issues
            .iter()
            .any(|i| i.path.contains("actor") || i.message.contains("agentId")),
        "an issue must point at the agent block or name agentId; got {issues:?}",
    );
}

#[test]
fn validate_metadata_rejects_human_missing_user_id() {
    let block = json!({
        "actor": { "type": "human" }
    });
    let issues = validate_metadata(&block);
    assert!(!issues.is_empty(), "missing userId must produce issues",);
    assert!(
        issues
            .iter()
            .any(|i| i.path.contains("actor") || i.message.contains("userId")),
        "an issue must point at the actor block or name userId; got {issues:?}",
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

// ─── bundle integrity ─────────────────────────────────────────────────────

#[test]
fn validate_metadata_returns_dot_joined_paths() {
    // PRD: "Public function returns dot-joined paths, same shape as
    // `validate()` for event types". Pin via a single error whose
    // path travels through the actor sub-object.
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
    // Path should be dot-joined ("actor.promptTokens") if the
    // validator points at the field, or "actor" if it points at the
    // parent. Either way: no JSON Pointer artefacts (no leading `/`,
    // no `~1`/`~0` escapes).
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
