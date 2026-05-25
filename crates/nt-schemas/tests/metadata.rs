//! Parity tests for `nt_schemas::validate_metadata`.
//!
//! Pins the canonical `eventMetadataSchema` from `packages/schemas`
//! against the wire shapes the client emits. As of schemas v0.3.0
//! the metadata block holds four independently-optional sibling
//! namespaces — `actor`, `execution`, `initiator`, `extra` — between
//! `data` and `source` on the envelope; every v0.2.x payload (where
//! `actor` was mandatory) still validates unchanged.
//!
//! Fixtures cover the original actor cases (valid agent minimal /
//! full, valid human, missing agentId, missing userId, wrong
//! discriminator, extra field) plus the `thinkingEffort` enum
//! boundary, plus the v0.3.0 widening: `execution` (closed-enum
//! `location`), `initiator` (re-uses `actorSchema`), `extra`
//! (`record<string, unknown>` with non-empty namespace keys), and a
//! cross-namespace shape that exercises all four together.

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
///   - The error sits at one of the actor-schema-bearing namespaces
///     (currently `actor` and `initiator`) AND the message mentions
///     `oneOf` — the discriminated-union variant: when a required
///     sub-field is missing on the agent / human side, the validator
///     reports "not valid under any of the schemas listed in the
///     'oneOf' keyword" at the namespace level rather than naming
///     the missing sub-field directly. Accepting this anchor avoids
///     a false-negative without weakening the test to
///     `!issues.is_empty()`.
///
/// **Maintenance:** any future top-level namespace that re-uses
/// `actorSchema` must be appended to `ACTOR_SCHEMA_NAMESPACES`
/// below — otherwise its discriminated-union failures will surface
/// at the new path and the helper will false-negative.
const ACTOR_SCHEMA_NAMESPACES: &[&str] = &["actor", "initiator"];

fn names_field(issues: &[ValidationIssue], field: &str) -> bool {
    issues.iter().any(|i| {
        i.message.contains(&format!("\"{field}\""))
            || i.message.contains(&format!("'{field}'"))
            || i.path == field
            || i.path.ends_with(&format!(".{field}"))
            || (ACTOR_SCHEMA_NAMESPACES.contains(&i.path.as_str())
                && i.message.contains("oneOf"))
    })
}

#[test]
fn validate_metadata_accepts_empty_metadata_block() {
    // Schemas v0.3.0 made every top-level namespace optional, so an
    // empty `{}` is a valid metadata block (an event with no actor /
    // execution / initiator / extra information). Pinning this
    // positively guards against an over-eager future tightening that
    // would re-require `actor` and break v0.3.x callers who emit
    // unattributed envelopes by design (CI bots, internal cron, the
    // human branch with the credentials path turned off).
    let issues = validate_metadata(&json!({}));
    assert_eq!(
        issues,
        Vec::<ValidationIssue>::new(),
        "empty metadata block must validate clean under v0.3.0; got {issues:?}",
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
    //
    // The bait field `actors` is a deliberate near-typo of the real
    // `actor` namespace: a sloppy emitter that pluralised the key
    // would land here, so the pin doubles as a guard against that
    // class of typo.
    let block = json!({
        "actor": { "type": "agent", "agentId": "claude" },
        "actors": 42
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

// ─── v0.3.0 widened namespaces ────────────────────────────────────────────

// `execution` — closed `local`/`remote` enum on `location`, optional
// `machineId` / `workspace` / `workerName` / `attempt`. Lets emitters
// answer "where + how was this event produced".

#[test]
fn validate_metadata_accepts_execution_local_minimal() {
    let issues = validate_metadata(&json!({
        "execution": { "location": "local" }
    }));
    assert_eq!(
        issues,
        Vec::<ValidationIssue>::new(),
        "minimal local execution must validate clean; got {issues:?}",
    );
}

#[test]
fn validate_metadata_accepts_execution_remote_with_every_field() {
    // Full shape — workspace, machineId, workerName, and attempt all
    // populated. Pins that every documented optional field is
    // permitted by the schema (a regression that accidentally
    // tightened one of them would surface here).
    let issues = validate_metadata(&json!({
        "execution": {
            "location": "remote",
            "machineId": "runner-7",
            "workspace": "github://magic-ingredients/no-tickets@main",
            "workerName": "ci",
            "attempt": 2
        }
    }));
    assert_eq!(
        issues,
        Vec::<ValidationIssue>::new(),
        "full remote execution shape must validate clean; got {issues:?}",
    );
}

#[test]
fn validate_metadata_rejects_execution_location_outside_enum() {
    // `location` is a closed enum (`local` | `remote`). Anything
    // else fails closed — the canonical schema doesn't want emitters
    // inventing a third location and silently expanding the contract.
    // Anchored on the `location` field (mirror of the
    // `thinkingEffort` enum pin) so the test still catches a regress-
    // ion where the validator started reporting the failure at the
    // wrong path or with the wrong field name.
    let issues = validate_metadata(&json!({
        "execution": { "location": "edge" }
    }));
    assert!(
        !issues.is_empty(),
        "out-of-enum execution.location must produce issues",
    );
    assert!(
        issues
            .iter()
            .any(|i| i.path.contains("location") || i.message.contains("location")),
        "an issue must name the offending field; got {issues:?}",
    );
}

#[test]
fn validate_metadata_rejects_execution_non_object() {
    // `execution` is `type: object`. A string-where-object-expected
    // is a clear shape violation.
    let issues = validate_metadata(&json!({
        "execution": "local"
    }));
    assert!(
        !issues.is_empty(),
        "non-object execution must produce issues",
    );
}

#[test]
fn validate_metadata_rejects_extra_field_inside_execution() {
    // `execution` is `.strict()` like the actor variants — emitters
    // can't sneak in a hand-rolled field outside the documented
    // shape. Pinned separately from the top-level strictness
    // (rejects_extra_top_level_field) so a sloppy implementation
    // that strict-checks one level but not the namespace can't
    // pass.
    let issues = validate_metadata(&json!({
        "execution": {
            "location": "local",
            "rogue": "field"
        }
    }));
    assert!(
        !issues.is_empty(),
        "extra field inside execution must produce issues",
    );
}

#[test]
fn validate_metadata_rejects_execution_attempt_below_minimum() {
    // `attempt` is `{integer, minimum: 1}` — counting starts at 1,
    // not 0. A retry counter that off-by-ones into 0 or a signed
    // value that lands negative must fail closed, not silently
    // sit on the wire as a meaningless attempt index.
    let issues = validate_metadata(&json!({
        "execution": { "location": "local", "attempt": 0 }
    }));
    assert!(
        !issues.is_empty(),
        "attempt=0 (below minimum 1) must produce issues",
    );
}

#[test]
fn validate_metadata_rejects_execution_attempt_non_integer() {
    // `attempt` is `type: integer` — fractional values get rejected.
    // Defends against a publisher accidentally passing a float (e.g.
    // ms-based retry exponential as `1.5`) into an integer slot.
    let issues = validate_metadata(&json!({
        "execution": { "location": "local", "attempt": 1.5 }
    }));
    assert!(
        !issues.is_empty(),
        "fractional attempt must produce issues",
    );
}

#[test]
fn validate_metadata_rejects_execution_attempt_string() {
    // `attempt` is integer-typed, not string-typed. A stringly-
    // typed publisher would otherwise pass numeric-looking
    // strings (`"2"`) through without the validator catching them.
    let issues = validate_metadata(&json!({
        "execution": { "location": "local", "attempt": "two" }
    }));
    assert!(
        !issues.is_empty(),
        "string attempt must produce issues",
    );
}

// `initiator` — re-uses `actorSchema`, so callers can record the
// upstream actor that delegated work. Same agent/human discriminated
// union as `actor`.

#[test]
fn validate_metadata_accepts_initiator_human() {
    let issues = validate_metadata(&json!({
        "initiator": { "type": "human", "userId": "u-1" }
    }));
    assert_eq!(
        issues,
        Vec::<ValidationIssue>::new(),
        "human initiator must validate clean; got {issues:?}",
    );
}

#[test]
fn validate_metadata_accepts_initiator_agent() {
    let issues = validate_metadata(&json!({
        "initiator": {
            "type": "agent",
            "agentId": "github-actions",
            "model": "claude-opus-4-7"
        }
    }));
    assert_eq!(
        issues,
        Vec::<ValidationIssue>::new(),
        "agent initiator must validate clean; got {issues:?}",
    );
}

#[test]
fn validate_metadata_rejects_initiator_agent_missing_agent_id() {
    // Mirrors `validate_metadata_rejects_agent_missing_agent_id`
    // over the `initiator` namespace — proves the actor schema is
    // wired in equivalently on both sides, not just stubbed.
    let issues = validate_metadata(&json!({
        "initiator": { "type": "agent" }
    }));
    assert!(
        !issues.is_empty(),
        "initiator agent missing agentId must produce issues",
    );
    assert!(
        names_field(&issues, "agentId"),
        "an issue must specifically name agentId; got {issues:?}",
    );
}

#[test]
fn validate_metadata_rejects_initiator_human_missing_user_id() {
    // Mirrors `validate_metadata_rejects_human_missing_user_id` for
    // the `initiator` namespace. Closes the parity gap: both
    // discriminator branches must be enforced on initiator just
    // like on actor, not just one of them.
    let issues = validate_metadata(&json!({
        "initiator": { "type": "human" }
    }));
    assert!(
        !issues.is_empty(),
        "initiator human missing userId must produce issues",
    );
    assert!(
        names_field(&issues, "userId"),
        "an issue must specifically name userId; got {issues:?}",
    );
}

#[test]
fn validate_metadata_rejects_extra_field_inside_initiator() {
    // Mirrors `validate_metadata_rejects_extra_field_inside_actor`.
    // `initiator` re-uses `actorSchema`, so the same `.strict()`
    // discipline applies — a sloppy implementation that strict-
    // checks the actor branch but not the initiator branch would
    // surface here.
    let issues = validate_metadata(&json!({
        "initiator": {
            "type": "agent",
            "agentId": "claude",
            "wat": "bonus"
        }
    }));
    assert!(
        !issues.is_empty(),
        "extra field inside initiator must produce issues",
    );
}

// `extra` — `record<string (min 1), unknown>`. The outer shape is
// pinned (object only, no primitives or arrays); the inside is
// opaque so emitters own their per-tool taxonomy.

#[test]
fn validate_metadata_accepts_extra_with_namespaced_payload() {
    let issues = validate_metadata(&json!({
        "extra": {
            "github.com/magic-ingredients/no-tickets-cli": {
                "version": "0.1.3",
                "channel": "stable"
            }
        }
    }));
    assert_eq!(
        issues,
        Vec::<ValidationIssue>::new(),
        "namespaced extras must validate clean; got {issues:?}",
    );
}

#[test]
fn validate_metadata_accepts_empty_extra_object() {
    // The schema treats `extra` as a record — an empty object is a
    // valid (zero-namespace) record. Distinct from "extra omitted":
    // both must validate.
    let issues = validate_metadata(&json!({ "extra": {} }));
    assert_eq!(
        issues,
        Vec::<ValidationIssue>::new(),
        "empty extra object must validate clean; got {issues:?}",
    );
}

#[test]
fn validate_metadata_rejects_extra_string() {
    // A primitive at `extra` breaks the record contract.
    let issues = validate_metadata(&json!({
        "extra": "some string"
    }));
    assert!(
        !issues.is_empty(),
        "string at extra must produce issues",
    );
}

#[test]
fn validate_metadata_rejects_extra_array() {
    // Arrays aren't records — same closure as the string case.
    let issues = validate_metadata(&json!({
        "extra": ["one", "two"]
    }));
    assert!(
        !issues.is_empty(),
        "array at extra must produce issues",
    );
}

#[test]
fn validate_metadata_rejects_extra_empty_namespace_key() {
    // `extra` is `record<string (min 1), unknown>` — namespace keys
    // must be non-empty. An empty key is the structurally-valid-
    // but-meaningless case: it's an object with one property, so
    // the outer type-check passes, but the canonical schema enforces
    // a `propertyNames: { minLength: 1 }` filter to make sure every
    // namespace actually identifies its owner. Pinning this guards
    // against an emitter accidentally writing `extra: { "": ... }`
    // when it meant `extra: { "tool-name": ... }`.
    let issues = validate_metadata(&json!({
        "extra": { "": { "version": "0.1.3" } }
    }));
    assert!(
        !issues.is_empty(),
        "empty namespace key in extra must produce issues",
    );
}

// Cross-namespace: prove all four siblings co-exist on one block.
// A schema regression that re-introduced a "mutually exclusive" or
// "oneOf at the top level" constraint would surface here.

#[test]
fn validate_metadata_accepts_all_four_namespaces_together() {
    let issues = validate_metadata(&json!({
        "actor": {
            "type": "agent",
            "agentId": "claude",
            "sessionId": "sess-1"
        },
        "execution": {
            "location": "remote",
            "machineId": "runner-7"
        },
        "initiator": {
            "type": "human",
            "userId": "andy"
        },
        "extra": {
            "github.com/some-tool": { "flag": true }
        }
    }));
    assert_eq!(
        issues,
        Vec::<ValidationIssue>::new(),
        "all four namespaces together must validate clean; got {issues:?}",
    );
}

// ─── bundle integrity ────────────────────────────────────────────────────

#[test]
fn validate_metadata_rejects_outright_when_schema_loaded() {
    // Sentinel: prove the metadata validator is non-trivially loaded
    // (not e.g. a "default-accepts-everything" fallback) by sending
    // a payload that NO reasonable schema would accept. v0.3.0
    // widened the metadata block to four optional namespaces — `{}`
    // is now valid by design — so the sentinel uses an `actor` whose
    // shape is clearly wrong (a string where an object/discriminated
    // union is required). If this passes, the metadataSchema field
    // is probably missing from the bundle. Companion to
    // `bundle_contains_every_expected_type_id` over in tests/validate.rs.
    let issues = validate_metadata(&json!({
        "actor": "not-an-object"
    }));
    assert!(
        !issues.is_empty(),
        "metadata schema must be loaded and strict — actor-as-string should fail; \
         if this passes, the metadataSchema field is probably missing from the bundle",
    );
}
