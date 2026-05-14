---
id: schemas-and-nt-session
prd_id: event-actor-metadata
number: 1
title: Schemas + `nt session` + `nt publish` actor wiring
status: not_started
created: 2026-05-14
updated: 2026-05-14
---

# Feature: Schemas + `nt session` + `nt publish` actor wiring

## Description

Phase 1 of the actor-metadata rollout. Ships the canonical schemas (TS + Rust), the `nt session` subcommand family, and the actor-resolution path inside `nt publish`. Server-side schema is updated to **accept** `metadata` as an optional field, so this phase is non-breaking and can roll out without coordinating a server cutover.

By the end of this feature, an agent harness can run `nt session start --agent claude --model claude-opus-4-7` once and every subsequent `nt publish` in that environment carries a fully-validated `metadata.actor` block automatically.

## Acceptance Criteria

- [ ] `actorSchema`, `agentActorSchema`, `humanActorSchema`, `eventMetadataSchema` exported from `@magic-ingredients/no-tickets-schemas` with TypeScript types
- [ ] Rust `nt-schemas` crate exposes `validate_metadata(metadata: &Value) -> Option<Vec<ValidationIssue>>` with TS-parity issue shape
- [ ] TS↔Rust validator parity test passes for: valid agent, valid human, invalid (missing mandatory), invalid (wrong discriminator), strict-mode extra-field rejection
- [ ] `eventEnvelopeSchema` in `notickets-service` accepts `metadata` as optional (no breaking change yet)
- [ ] `nt session start` writes `~/.notickets/active-session.json` atomically (temp + rename) with the resolved actor block plus `startedAt` / `pid`
- [ ] `nt session show` prints the active session or "no active session"; flags expiry when stale (>24h or beyond `--max-age-hours`)
- [ ] `nt session end` deletes the file; idempotent (no error when no session exists)
- [ ] `nt publish` actor resolution implements the documented precedence chain: flags > `NT_SESSION_FILE` > `active-session.json` > credentials > exit 5 (not_authenticated)
- [ ] `EventEnvelope` struct serialises `metadata` between `data` and `source`, omitting the field when no actor resolved (Phase 1 server tolerance only)
- [ ] `nt publish` flags: `--actor-type`, `--agent-id`, `--model`, `--provider`, `--thinking-effort`, `--session-id`, `--call-id`, `--prompt-tokens`, `--completion-tokens`, `--latency-ms`, `--session-file`
- [ ] All existing `nt publish` integration tests pass; new actor-wiring tests cover every precedence-chain branch

## Tasks

### 1. Define actor + metadata schemas in `packages/schemas`
status: not_started

Add the canonical `agentActorSchema`, `humanActorSchema`, `actorSchema` (discriminated union on `type`), and `eventMetadataSchema` to `packages/schemas`. Export both schemas and their inferred types. The schemas live at the package top level — they are envelope concerns, not per-event-type concerns.

**Files to modify/create:**
- `packages/schemas/src/metadata/actor.ts` (new)
- `packages/schemas/src/metadata/index.ts` (new)
- `packages/schemas/src/index.ts` — re-export
- `packages/schemas/src/metadata/actor.test.ts` (new)

**Expected changes:**
- Strict zod objects on each variant; discriminated union by `type`
- Mandatory: agent.{agentId, model}; human.{userId}
- Optional agent fields: provider, sessionId, callId, thinkingEffort (enum low/medium/high), promptTokens, completionTokens, latencyMs
- Optional human fields: email
- Top-level `eventMetadataSchema = z.object({ actor: actorSchema }).strict()`
- Unit tests cover both variants, all mandatory-field violations, strict-mode rejection of unknown fields, enum boundaries for thinkingEffort

### 2. Make `metadata` optional on the server envelope schema
status: not_started

Add `metadata: eventMetadataSchema.optional()` to `eventEnvelopeSchema` in the server's `publish-batch.ts`. Optional in this phase so existing callers don't break. When provided, metadata is validated. When absent, the server records the event with `metadata = NULL` (Phase 2 adds the column).

**Files to modify/create:**
- `packages/notickets-service/src/server/events/publish-batch.ts`
- `packages/notickets-service/src/server/events/publish-batch.test.ts`

**Expected changes:**
- Add optional `metadata` field referencing the schemas-package export
- Test: envelope with valid metadata passes; envelope with invalid metadata fails with 422; envelope without metadata still passes (Phase 1 tolerance)

### 3. Wire `validate_metadata` into the Rust `nt-schemas` crate
status: not_started

Extend the Rust validator with `validate_metadata(metadata: &Value) -> Option<Vec<ValidationIssue>>` matching the TS shape. Pin parity via a fixture-driven test that exercises the same payloads through both languages.

**Files to modify/create:**
- `crates/nt-schemas/src/lib.rs`
- `crates/nt-schemas/tests/metadata.rs` (new)
- `crates/nt-schemas/tests/parity-fixtures/` (new, vendored JSON payloads + expected issues)

**Expected changes:**
- Embed the metadata JSON Schema from the same release-artifact bundle (`build.rs` already fetches it; bundle gains a `metadataSchema` top-level entry — coordinate with sister fix's `build-json-schema-bundle.ts`)
- Public function returns dot-joined paths, same shape as `validate()` for event types
- Parity fixtures: at least 6 cases (valid agent, valid human, missing agentId, missing userId, wrong discriminator, extra field)

### 4. Implement `nt session start / show / end` subcommands
status: not_started

Add the `session` command group to `nt-cli`. `start` resolves and atomically writes `~/.notickets/active-session.json`. `show` prints the active session as JSON or reports "no active session"; flags staleness explicitly. `end` deletes the file idempotently.

**Files to modify/create:**
- `crates/nt-cli/src/commands/session.rs` (new)
- `crates/nt-cli/src/commands/mod.rs`
- `crates/nt-cli/src/main.rs` — register subcommand
- `crates/nt-cli/src/session.rs` (new) — pure read/write/validate of the session file
- `crates/nt-cli/tests/session.rs` (new) — `assert_cmd`-driven integration tests

**Expected changes:**
- Flags on `start`: `--agent` (required), `--model` (required), `--provider`, `--thinking-effort`, `--session-id`, `--max-age-hours`
- Atomic write via temp + rename
- File schema: `{ version: 1, actor: {…}, startedAt: ISO8601, pid: number, maxAgeHours: number }`
- `show` flags `expired: true` when current time > startedAt + maxAgeHours
- `end` exits 0 when no session present (idempotent)
- Tests: round-trip start→show→publish-stamp→end; stale detection; concurrent start replaces atomically; show on no-session prints `{"active":false}`

### 5. Add actor resolution + `metadata` emission to `nt publish`
status: not_started

`nt publish` resolves the actor block per the precedence chain (flags > `NT_SESSION_FILE` > `active-session.json` > credentials > error). The resolved actor is serialised into the envelope's `metadata` field between `data` and `source`. New flags cover every actor field for one-off overrides.

**Files to modify/create:**
- `crates/nt-cli/src/commands/publish.rs`
- `crates/nt-cli/src/actor.rs` (new) — pure resolver function
- `crates/nt-cli/tests/publish.rs` — extend the wiremock suite
- `crates/nt-cli/tests/actor-resolution.rs` (new) — table-driven precedence tests

**Expected changes:**
- New flags: `--actor-type` (human|agent), `--agent-id`, `--model`, `--provider`, `--thinking-effort`, `--session-id`, `--call-id`, `--prompt-tokens`, `--completion-tokens`, `--latency-ms`, `--session-file`
- `EventEnvelope` struct gains `metadata: Option<Metadata>`; serialises in declaration order between `data` and `source`
- Per-call fields (callId, tokens, latency) layer on top of session-resolved defaults
- Exit code 5 (`not_authenticated`) with structured-error JSON when no actor can be resolved
- Wire-format test pins field order: `type, data, metadata, source, …`
- Wiremock fixtures updated to expect `metadata` on the request body
- Table-driven precedence tests cover all five branches of the chain

### 6. Document the public binary contract for `metadata`
status: not_started

Update `docs/binary-stream-protocol.md` (per `cross-platform-cli-binary` Task 4b's plan) and add a section to the publish reference describing the `--actor-*` flags, the `nt session` lifecycle, and the resolution precedence. Include a short cookbook for the three common harness shapes (single agent boot, multi-agent host with `NT_SESSION_FILE`, human-CLI default).

**Files to modify/create:**
- `docs/cli-reference.md` — extend the `publish` section, add `session` section
- `docs/cookbook/actor-resolution.md` (new)
- `docs/rust-spike-notes.md` — append Phase 1 notes

**Expected changes:**
- Resolution precedence diagram
- Worked examples per harness shape
- Explicit non-goals: no prompt content, no completion text, no tool-call args

## Dependencies

- **`cross-platform-cli-binary` fix, Task 4 (full CLI port)**: this feature's CLI work plugs into the same surface. Task 4 must land first or in parallel — the new subcommand group can't be added before `clap` derive scaffolding exists.
- **`cross-platform-cli-binary` fix, Task 4a (structured-error contract)**: the `not_authenticated` exit code (5) must exist before this feature can return it.
- **Sister fix `client-roadmap-server-prerequisites`**: the published JSON Schema bundle (Tasks 6+7) needs a `metadataSchema` top-level entry. Coordinate the one-line addition to `build-json-schema-bundle.ts` so `crates/nt-schemas/build.rs` can fetch it.

## Testing Strategy

### Unit Tests

- TS: every actor variant validates / rejects per spec; strict-mode rejects extras; thinkingEffort enum boundary
- Rust: `nt-schemas::validate_metadata` returns identical issue shapes to TS for the same payloads (parity fixtures)
- Rust: `actor::resolve()` is a pure function over (flags, env, file-system-mock); table-driven coverage of all five precedence branches
- Rust: session file write is atomic — temp file exists, then rename, never half-written

### Integration Tests

- `nt session start` writes a parseable file; `nt session show` reads it back identically; `nt session end` deletes it
- Round-trip: `nt session start … && nt publish --type ai.completion.recorded.v1 --data '{...}'` → server receives envelope with the expected `metadata.actor`
- Stale session: simulate `startedAt = now - 25h` → `nt session show` reports expired; `nt publish` falls back to credentials path
- `NT_SESSION_FILE` env var: point at an alternate file → resolution uses it instead of the default

### Manual Testing

- Two parallel terminals: one runs `nt session start --agent claude`, the other runs `nt session start --agent codex --session-file /tmp/codex-sess.json`. Verify each `nt publish` resolves the correct actor.
- Real wire smoke: with `NO_TICKETS_TOKEN` exported, run `nt session start … && nt publish` against staging; confirm the event lands with `metadata.actor` populated (Phase 1 server accepts; Phase 2 will store).

## Implementation Notes

- The session file is small (<1 KB). No need for a streaming parser; full read + JSON parse is fine.
- Atomic write order: open temp in same directory as the destination (so rename stays atomic on Linux/macOS); fsync the temp; rename. Document this pattern with a comment — future contributors may not remember why.
- `pid` is recorded for diagnostic visibility but is **not** consulted when deciding session validity in v1 — cross-platform pid liveness checks are messy. Show it in `nt session show` so a confused operator can see "oh, that session was started by pid 1234 which is gone."
- The resolution function should accept injected `Env`, `Clock`, and `FileSystem` traits so unit tests don't touch real env/disk/time. Mirror the existing pattern in `crates/nt-cli/src/env.rs`.
- Wire-format field-order test: the wiremock body assertion uses the `monotonic-byte-position` helper already in `tests/publish.rs`. Add a fixture line `"metadata":` between `"data":` and `"source":` and pin its position.

## Workflow Example

```bash
# Agent harness boot
nt session start \
  --agent claude \
  --model claude-opus-4-7 \
  --provider anthropic \
  --thinking-effort high \
  --session-id sess-$(uuidgen)

# Confirm
nt session show
# → {"active": true, "actor": {...}, "startedAt": "...", "expired": false}

# Every subsequent publish auto-stamps metadata.actor
nt publish --type product.feature.status_changed.v1 \
  --data '{"featureId":"f-1","fromStatus":"review","toStatus":"done"}' \
  --call-id call-$(uuidgen) \
  --prompt-tokens 1234 \
  --completion-tokens 567

# On harness teardown
nt session end
```

## Benefits

- Agents declare identity once, not per publish
- LLM metadata stays out of shell env state
- Server gets full actor accountability with no coordinated cutover (Phase 1 is non-breaking)
- Foundation for Phase 2's hard requirement + Phase 3's UI rendering
