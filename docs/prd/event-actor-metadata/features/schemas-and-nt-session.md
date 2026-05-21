---
id: schemas-and-nt-session
prd_id: event-actor-metadata
number: 1
title: Schemas + `no-tickets session` + `no-tickets publish` actor wiring
status: not_started
created: 2026-05-14
updated: 2026-05-14
---

# Feature: Schemas + `no-tickets session` + `no-tickets publish` actor wiring

## Description

Phase 1 of the opt-in actor-metadata rollout. Ships the canonical schemas (TS + Rust), the `no-tickets session` subcommand family, the actor-resolution path inside `no-tickets publish`, and the first-publish hint mechanic. Server-side schema is updated to **accept** `metadata` as an optional field, and it stays optional permanently — there is no later phase that flips this.

By the end of this feature, an agent harness can run `no-tickets session start --agent claude --model claude-opus-4-7` once and every subsequent `no-tickets publish` in that environment carries a fully-validated `metadata.actor` block automatically. Callers who don't run `session start` publish unattributed events; the CLI prints a one-time hint on the first such publish telling them how to opt in, then stays silent.

## Acceptance Criteria

- [ ] `actorSchema`, `agentActorSchema`, `humanActorSchema`, `eventMetadataSchema` exported from `@magic-ingredients/no-tickets-schemas` with TypeScript types. Agent schema's mandatory field is `agentId` only; `model` is optional.
- [ ] Rust `nt-schemas` crate exposes `validate_metadata(metadata: &Value) -> Option<Vec<ValidationIssue>>` with TS-parity issue shape
- [ ] TS↔Rust validator parity test passes for: valid agent (agentId only), valid agent (agentId + model + extras), valid human, invalid (missing mandatory `agentId` / `userId`), invalid (wrong discriminator), strict-mode extra-field rejection
- [ ] `eventEnvelopeSchema` in `notickets-service` accepts `metadata` as optional. Stays optional — no later phase flips this.
- [ ] `no-tickets session start` writes `<config-dir>/active-session.json` atomically (temp + rename) with the resolved actor block plus `startedAt` / `pid`. `<config-dir>` resolves via `paths::config_dir(env)` (ADR-0002).
- [ ] `no-tickets session start` requires only `--agent`. `--model` and all other LLM-context flags are optional and omitted from the actor block when not supplied. No sentinel values like `"n/a"` are accepted or emitted anywhere in the pipeline.
- [ ] `no-tickets session show` prints the active session or `{"active": false}`; flags expiry when stale (>24h or beyond `--max-age-hours`)
- [ ] `no-tickets session end` deletes the active-session file **and** clears `firstPublishHintShown` from `<config-dir>/state.json`; idempotent (no error when neither exists)
- [ ] `no-tickets publish` actor resolution implements the documented precedence chain: flags > `NO_TICKETS_SESSION_FILE` > `active-session.json` > credentials > **unattributed publish (with one-time hint)**. There is no error branch; publish always succeeds when the envelope is otherwise valid.
- [ ] First-publish hint: when resolution lands on the no-actor branch and `<config-dir>/state.json` does not have `firstPublishHintShown: true`, the CLI prints the hint to stderr and atomically updates the file to set the flag. `--quiet` flag and `NO_TICKETS_QUIET=1` env var suppress the stderr output but still set the flag.
- [ ] `EventEnvelope` struct serialises `metadata` between `data` and `source`, omitting the field entirely when no actor resolved
- [ ] `no-tickets publish` flags: `--actor-type`, `--agent-id`, `--model`, `--provider`, `--thinking-effort`, `--session-id`, `--call-id`, `--prompt-tokens`, `--completion-tokens`, `--latency-ms`, `--session-file`, `--quiet`
- [ ] All existing `no-tickets publish` integration tests pass; new actor-wiring tests cover every precedence-chain branch including the hint-marker write/clear semantics

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
- Mandatory: `agent.agentId`; `human.userId`. Nothing else is mandatory.
- Optional agent fields: `model`, `provider`, `sessionId`, `callId`, `thinkingEffort` (enum low/medium/high), `promptTokens`, `completionTokens`, `latencyMs`
- Optional human fields: `email`
- Top-level `eventMetadataSchema = z.object({ actor: actorSchema }).strict()`
- Unit tests cover: both variants with mandatory-only fields, both variants with all fields, missing `agentId` / `userId` rejection, wrong discriminator rejection, strict-mode rejection of unknown fields, enum boundaries for `thinkingEffort`. **No test sets `model: 'n/a'`** — the field is omitted when not applicable.

### 2. Make `metadata` optional on the server envelope schema (permanently)

status: not_started

Add `metadata: eventMetadataSchema.optional()` to `eventEnvelopeSchema` in the server's `publish-batch.ts`. Optional, permanently — no later phase removes the `.optional()`. When provided, metadata is validated. When absent, the server accepts the envelope; Phase 2 adds the column and persists `NULL` for absent metadata.

**Files to modify/create:**
- `packages/notickets-service/src/server/events/publish-batch.ts`
- `packages/notickets-service/src/server/events/publish-batch.test.ts`

**Expected changes:**
- Add optional `metadata` field referencing the schemas-package export
- Test: envelope with valid metadata passes; envelope with invalid metadata fails with 422; envelope without metadata passes (and this is a permanent contract, not a tolerance window)

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
- Parity fixtures: at least 7 cases — valid agent (agentId only, no model), valid agent (agentId + model + extras), valid human, missing `agentId`, missing `userId`, wrong discriminator, extra field. The two agent-valid cases pin that `model` is optional in both bindings.

### 4. Implement `no-tickets session start / show / end` subcommands
status: completed
commitSha: ddaac08

Add the `session` command group to `nt-cli`. `start` resolves and atomically writes `<config-dir>/active-session.json`. `show` prints the active session as JSON or `{"active": false}`; flags staleness explicitly. `end` deletes the active-session file **and** clears the `firstPublishHintShown` flag in `<config-dir>/state.json`, both idempotently.

`<config-dir>` is `paths::config_dir(env)` per ADR-0002 — platform-native via `directories::ProjectDirs`, overridable to `<dir>/.notickets/` via `NO_TICKETS_HOME=<dir>`.

**Files to modify/create:**
- `crates/nt-cli/src/commands/session.rs` (new)
- `crates/nt-cli/src/commands/mod.rs`
- `crates/nt-cli/src/main.rs` — register subcommand
- `crates/nt-cli/src/session.rs` (new) — pure read/write/validate of the session file
- `crates/nt-cli/src/state.rs` (new) — pure read/write of `state.json` (hint marker; future CLI state)
- `crates/nt-cli/tests/session.rs` (new) — `assert_cmd`-driven integration tests

**Expected changes:**
- Flags on `start`: `--agent` (required), `--model` (optional), `--provider` (optional), `--thinking-effort` (optional), `--session-id` (optional), `--max-age-hours` (optional). No flag is required besides `--agent`; omitted flags are omitted from the actor block, not stored as sentinel strings.
- Atomic write via temp + rename, into the same directory as the destination so rename stays atomic on Linux/macOS
- File schema: `{ version: 1, actor: {…}, startedAt: ISO8601, pid: number, maxAgeHours: number }`
- `show` flags `expired: true` when current time > startedAt + maxAgeHours
- `end` removes `active-session.json` if present and clears `firstPublishHintShown` from `state.json` (creating `state.json` only if it already exists with other state; never just to write a `false` flag). Exits 0 either way.
- Tests: round-trip `start → show → publish-stamp → end`; stale detection; concurrent `start` replaces atomically; `show` on no-session prints `{"active":false}`; `end` clears the hint marker so a subsequent unattributed publish re-prints the hint

### 5. Add actor resolution + `metadata` emission + first-publish hint to `no-tickets publish`
status: completed
commitSha: 7de873e

`no-tickets publish` resolves the actor block per the precedence chain (flags > `NO_TICKETS_SESSION_FILE` > `active-session.json` > credentials > unattributed). The resolved actor — when there is one — is serialised into the envelope's `metadata` field between `data` and `source`. When there isn't one, the field is omitted entirely and the first-publish hint mechanic runs.

**Files to modify/create:**
- `crates/nt-cli/src/commands/publish.rs`
- `crates/nt-cli/src/actor.rs` (new) — pure resolver function returning `Resolved { actor: Option<Actor>, source: ResolutionSource }`
- `crates/nt-cli/src/hint.rs` (new) — pure first-publish-hint logic (decide-if-fire, render text); doesn't touch IO
- `crates/nt-cli/tests/publish.rs` — extend the wiremock suite
- `crates/nt-cli/tests/actor-resolution.rs` (new) — table-driven precedence tests

**Expected changes:**
- New flags: `--actor-type` (human|agent), `--agent-id`, `--model`, `--provider`, `--thinking-effort`, `--session-id`, `--call-id`, `--prompt-tokens`, `--completion-tokens`, `--latency-ms`, `--session-file`, `--quiet` (suppresses hint stderr; still sets the marker)
- `EventEnvelope` struct gains `metadata: Option<Metadata>`; serialises in declaration order between `data` and `source`; field is **omitted from JSON output** when `None` (no `"metadata": null` on the wire)
- Per-call fields (callId, tokens, latency) layer on top of session-resolved defaults
- No exit-5 / `not_authenticated` error branch. Resolver returning `actor: None` is a normal outcome.
- When `actor: None` and `state.json` shows `firstPublishHintShown != true`: render the hint to stderr (unless `--quiet` / `NO_TICKETS_QUIET=1`) and atomically write `state.json` with the flag set. When `--quiet`, suppress stderr but still set the flag.
- Wire-format test pins field order: `type, data, metadata, source, …` (when metadata present); pins absent-metadata wire shape too
- Wiremock fixtures updated to expect `metadata` on the request body for session-attributed cases; absent for unattributed cases
- Table-driven precedence tests cover all five branches of the chain, plus hint-marker write + clear + idempotent-set semantics

### 6. Document the public binary contract for `metadata`
status: completed

Add a section to the publish reference describing the `--actor-*` flags, the `no-tickets session` lifecycle, the resolution precedence, and the first-publish hint. Include a short cookbook for the common harness shapes (single agent boot, multi-agent host with `NO_TICKETS_SESSION_FILE`, human-CLI default, deliberately-unattributed publish with `--quiet`).

**Files to modify/create:**
- `docs/cli-reference.md` — extend the `publish` section, add `session` section, document `--quiet` and `NO_TICKETS_QUIET`
- `docs/cookbook/actor-resolution.md` (new)
- `docs/rust-spike-notes.md` — append Phase 1 notes

**Expected changes:**
- Resolution precedence diagram, ending in the soft "unattributed + one-time hint" branch (not in an error)
- Worked examples per harness shape
- Explicit non-goals: no prompt content, no completion text, no tool-call args, no environment-based actor inference
- Cross-reference to `docs/fixes/stream-mode.md` for how `--stream` inherits the same actor resolution at process startup (extracted from `cross-platform-cli-binary`, owns its own scope)

## Dependencies

- **`cross-platform-cli-binary` fix (✅ completed 2026-05-20)**: Task 4 (full CLI port) provided the `clap` derive scaffolding the `session` subcommand group plugs into. Task 5 (MCP server port) is where the same actor resolver is wired into the MCP `publish` tool. ADR-0002 provides the platform-native config-dir resolution this feature uses for `active-session.json` and `state.json` placement.
- **`headless-init-device-code` fix (not_started)**: the human-actor fallback in the precedence chain (branch 4) only works on hosts where `no-tickets init` can run. Not a hard blocker for Feature 1 — agents that explicitly declare via `session start` don't need credentials — but `docs/cookbook/actor-resolution.md` should cross-reference the headless `init --device` path for callers who want the human fallback on remote / sandbox / CI hosts.
- **`stream-mode` fix (not_started, at `docs/fixes/stream-mode.md`)**: `--stream` reuses the same actor resolver at process startup. Coordinate so that the stream-mode protocol doc references the resolution precedence by reference rather than restating it.
- **Sister fix `client-roadmap-server-prerequisites`**: the published JSON Schema bundle (Tasks 6+7) needs a `metadataSchema` top-level entry. Coordinate the one-line addition to `build-json-schema-bundle.ts` so `crates/nt-schemas/build.rs` can fetch it.

## Testing Strategy

### Unit Tests

- TS: every actor variant validates / rejects per spec; strict-mode rejects extras; thinkingEffort enum boundary; agent-with-just-`agentId` validates (no `model`)
- Rust: `nt-schemas::validate_metadata` returns identical issue shapes to TS for the same payloads (parity fixtures)
- Rust: `actor::resolve()` is a pure function over (flags, env, file-system-mock); table-driven coverage of all five precedence branches, including the no-actor branch's two sub-cases (marker set / marker unset)
- Rust: `hint::should_fire()` is pure over `(resolved: Resolved, state: State, quiet: bool)` — returns `(emit_stderr, set_marker)` deterministically
- Rust: session file write is atomic — temp file exists, then rename, never half-written. Same for `state.json`.

### Integration Tests

- `no-tickets session start` writes a parseable file; `no-tickets session show` reads it back identically; `no-tickets session end` deletes it and clears `firstPublishHintShown`
- Round-trip: `no-tickets session start … && no-tickets publish --type ai.completion.recorded.v1 --data '{...}'` → server receives envelope with the expected `metadata.actor`; no hint fires
- Stale session: simulate `startedAt = now - 25h` → `no-tickets session show` reports expired; `no-tickets publish` falls back to credentials path
- `NO_TICKETS_SESSION_FILE` env var: point at an alternate file → resolution uses it instead of the default
- **No session + no creds, first publish:** envelope is sent **without `metadata`** (server accepts); stderr contains the hint; `state.json` now shows `firstPublishHintShown: true`
- **No session + no creds, second publish:** envelope sent without `metadata`; stderr is silent; `state.json` unchanged
- **`--quiet` on first unattributed publish:** stderr silent; marker still set
- **`session end` after a marker was set:** `state.json` has the flag cleared; the next unattributed publish re-fires the hint
- **Session-attributed paths touch no `state.json`:** recording-FS test asserts zero opens on `state.json` when resolution lands on branches 1–4

### Manual Testing

- Two parallel terminals: one runs `no-tickets session start --agent claude`, the other runs `no-tickets session start --agent codex --session-file /tmp/codex-sess.json`. Verify each `no-tickets publish` resolves the correct actor.
- Real wire smoke: with `NO_TICKETS_TOKEN` exported, run `no-tickets session start … && no-tickets publish` against staging; confirm the event lands with `metadata.actor` populated.
- Hint smoke: with a fresh `<config-dir>` (or after `no-tickets session end`), run `no-tickets publish --type … --data …` with no session and no credentials. Confirm stderr prints the one-time hint, the event lands without `metadata`, and a second invocation prints nothing.

## Implementation Notes

- The session file is small (<1 KB). No need for a streaming parser; full read + JSON parse is fine.
- Atomic write order: open temp in same directory as the destination (so rename stays atomic on Linux/macOS); fsync the temp; rename. Document this pattern with a comment — future contributors may not remember why. Same pattern for `state.json`.
- `pid` is recorded for diagnostic visibility but is **not** consulted when deciding session validity in v1 — cross-platform pid liveness checks are messy. Show it in `no-tickets session show` so a confused operator can see "oh, that session was started by pid 1234 which is gone."
- The resolution function should accept injected `Env`, `Clock`, and `FileSystem` traits so unit tests don't touch real env/disk/time. Mirror the existing pattern in `crates/nt-cli/src/env.rs`.
- Wire-format field-order test: the wiremock body assertion uses the `monotonic-byte-position` helper already in `tests/publish.rs`. Add a fixture line `"metadata":` between `"data":` and `"source":` and pin its position. Add a complementary test that asserts absence — `metadata` key must not appear in the request body when the resolver returns `None`.

### Hint mechanic — overhead and gating

Three properties the implementer must preserve. Tests should pin each.

1. **State check is gated on the no-actor branch.** The resolver checks `state.json` *only* when resolution lands on branch 5 (no flags, no `NO_TICKETS_SESSION_FILE`, no fresh `active-session.json`, no credentials). Branches 1–4 short-circuit before any `state.json` IO. Concretely: the happy paths — caller has a session, caller has flags, caller has credentials — perform **zero** `state.json` syscalls per publish. Pin this with a test that uses a recording `FileSystem` trait and asserts `state.json` is never touched on the session-attributed paths.

2. **Hint fires at most once per unit of work, never per event.** Resolution runs once per CLI invocation. In single-publish mode, that's once per `no-tickets publish` invocation. In batch mode (`--file` or stdin), it's once per batch invocation — the resolved actor (or its absence) is computed once, then applied to every event in the batch. The hot publish loop must not re-enter the resolver. Pin this with a batch-mode test that submits N events and asserts `state.json` was opened exactly once during the invocation.

3. **`--stream` resolves at process startup, then never.** When `--stream` mode lands (tracked in `docs/fixes/stream-mode.md`), the resolver runs once before reading the first stdin JSON line; the resolved actor (or `None`) is cached for the stream's lifetime. The hint, if it fires, is printed to stderr **before any event JSON flows** — never mid-stream — and the marker is set immediately. Subsequent stdin lines reuse the cached resolution; per-event overrides (`callId`, prompt tokens, etc.) layer on top via the JSONL line itself per `stream-mode.md`'s protocol. Pin this in Feature 1 by exposing the resolver as a pure function whose output is trivially cacheable; the actual `--stream` integration test belongs to the stream-mode fix, but Feature 1 must not foreclose this shape.

In all three paths, an unattributed publish costs ~20 μs of `state.json` work in steady state (after the marker is set), and ~100 μs on the first invocation that writes the marker. Negligible against a ~10–50 ms publish round-trip. `--quiet` and `NO_TICKETS_QUIET=1` suppress the stderr hint output but still set the marker, so the env-var doesn't have to stay set forever once the operator has decided.

## Workflow Example

```bash
# Agent harness boot
no-tickets session start \
  --agent claude \
  --model claude-opus-4-7 \
  --provider anthropic \
  --thinking-effort high \
  --session-id sess-$(uuidgen)

# Confirm
no-tickets session show
# → {"active": true, "actor": {...}, "startedAt": "...", "expired": false}

# Every subsequent publish auto-stamps metadata.actor
no-tickets publish --type product.feature.status_changed.v1 \
  --data '{"featureId":"f-1","fromStatus":"review","toStatus":"done"}' \
  --call-id call-$(uuidgen) \
  --prompt-tokens 1234 \
  --completion-tokens 567

# On harness teardown
no-tickets session end
```

## Benefits

- Agents declare identity once, not per publish
- LLM metadata stays out of shell env state
- Server gets actor information for opt-in callers with no coordinated cutover (this phase is non-breaking, and so are all subsequent phases)
- Foundation for Phase 2's storage and Phase 3's UI rendering
- Callers who haven't opted in are never broken — they get a one-time discoverability hint, then the CLI stays out of their way
