---
id: per-language-wrappers
prd_id: event-actor-metadata
number: 4
title: Per-language wrappers inherit `no-tickets session`
status: not_started
created: 2026-05-14
updated: 2026-05-21
---

# Feature: Per-language wrappers inherit `no-tickets session`

## Description

Phase 4 of the opt-in actor-metadata rollout. Pairs with the per-language wrappers (TS / Python / Go, each ~50–80 LOC over the `no-tickets` binary): when a wrapper's constructor is given actor config, the wrapper calls `no-tickets session start` at init and `no-tickets session end` at shutdown. When the constructor is given no actor config, the wrapper publishes unattributed — exactly like the binary's no-session path — and the binary's one-time hint reaches the wrapper's stderr.

The whole point of the wrapper model is that the binary owns the integration logic. Actor resolution is one of the things that gets resolved once in `no-tickets` rather than reimplemented per language. This feature is mostly conformance — proving each wrapper does the lifecycle handshake correctly **when asked to** and stays out of the way otherwise — plus a small `withActor()` convenience for callers who want per-call overrides without poking at the session file.

TS-SDK survival is resolved (ADR-0003, 2026-05-20): the TS wrapper returns as the `@magic-ingredients/no-tickets` npm package with split exports (`/sdk` for markdown helpers, `/client` for the spawn-based client). The TS wrapper in this feature lives behind the `/client` entry.

If the wrappers slip, this feature slips with them — the actor model is fully functional through `no-tickets` direct invocation without per-language wrappers.

## Acceptance Criteria

- [ ] TS wrapper (`@magic-ingredients/no-tickets`, `/client` export): when constructor receives actor config, spawns `no-tickets session start` on first use and `no-tickets session end` on process exit / explicit close. When no actor config is passed, no `session` subprocesses are spawned.
- [ ] Python wrapper does the same lifecycle dance via `subprocess.run` + `atexit` when actor config is passed; no-op when it isn't
- [ ] Go wrapper does the same lifecycle dance via `os/exec` + deferred cleanup when actor config is passed; no-op when it isn't
- [ ] Each wrapper exposes a `withActor(overrides, fn)` API for one-off actor overrides on a single publish without rewriting the session file
- [ ] Each wrapper's conformance test: with actor config, assert the wire payload contains the expected `metadata.actor`; without actor config, assert the wire payload **omits** `metadata`
- [ ] Each wrapper documents the actor model in its README — what gets inherited (when opted in), what can be overridden, how `no-tickets session end` is automated, and what happens when actor config is omitted
- [ ] Wrappers do NOT reimplement actor resolution. They pass values into `no-tickets` flags or rely on the session file `no-tickets` already manages
- [ ] Wrappers do NOT sniff environment variables to invent actor config. If the caller wanted attribution, they pass it to the constructor explicitly

## Tasks

### 1. TS wrapper: spawn `no-tickets session start` + `withActor` API
status: not_started

The TS wrapper (`@magic-ingredients/no-tickets`, ~50–80 LOC over spawn-glue) gains a session lifecycle. First call to `publish()` spawns `no-tickets session start` with values from the wrapper's constructor (`new NoTickets({ agentId, model, … })`). Process exit hook (`process.on('beforeExit')`) spawns `no-tickets session end`. The `withActor(overrides, fn)` helper wraps a callback so its publishes carry actor-override flags.

**Files to modify/create:**
- `wrappers/typescript/src/session.ts` (new)
- `wrappers/typescript/src/index.ts`
- `wrappers/typescript/src/with-actor.ts` (new)
- `wrappers/typescript/test/session.test.ts` (new)
- `wrappers/typescript/test/conformance.test.ts` (new)

**Expected changes:**
- Constructor accepts `{ agentId, model, provider?, thinkingEffort?, sessionId? }` and spawns `no-tickets session start` lazily on first publish
- `process.on('beforeExit')` invokes `no-tickets session end`
- `withActor({ callId, promptTokens, … }, async () => { await client.publish(…) })` threads override flags into the underlying `no-tickets publish`
- Conformance test asserts the wire body (captured via wiremock) contains the expected `metadata.actor`

### 2. Python wrapper: spawn `no-tickets session start` + `with_actor` API
status: not_started

Python equivalent of Task 1. Uses `subprocess.run` for short-lived spawns (and `subprocess.Popen` later when stream-mode lands per `docs/fixes/stream-mode.md`). Session lifecycle hooks tie to `atexit.register` and an optional context manager (`with NoTickets(agent_id=…) as client:`).

**Files to modify/create:**
- `wrappers/python/no_tickets/session.py` (new)
- `wrappers/python/no_tickets/__init__.py`
- `wrappers/python/no_tickets/with_actor.py` (new)
- `wrappers/python/tests/test_session.py` (new)
- `wrappers/python/tests/test_conformance.py` (new)

**Expected changes:**
- `NoTickets(agent_id=…, model=None, provider=None, thinking_effort=None, session_id=None)` constructor — only `agent_id` is required when actor config is supplied; `NoTickets()` with no args is a valid no-actor construction
- Context-manager protocol calls `no-tickets session start` on `__enter__` **only when `agent_id` was supplied** and `no-tickets session end` on `__exit__` likewise
- `with_actor({"call_id": …, "prompt_tokens": …})` as a decorator and a context manager
- Conformance test via a fake HTTP server (wiremock-equivalent in Python — `pytest-httpserver`); covers both attributed and unattributed shapes

### 3. Go wrapper: spawn `no-tickets session start` + `WithActor` API
status: not_started

Go equivalent of Task 1. `os/exec.Cmd` for spawns. Session lifecycle via a `Close()` method on the client; idiomatic Go callers use `defer client.Close()`.

**Files to modify/create:**
- `wrappers/go/notickets/session.go` (new)
- `wrappers/go/notickets/client.go`
- `wrappers/go/notickets/with_actor.go` (new)
- `wrappers/go/notickets/session_test.go` (new)
- `wrappers/go/notickets/conformance_test.go` (new)

**Expected changes:**
- `notickets.New(notickets.Config{AgentID: …, Model: …, …})` constructor calls `no-tickets session start`
- `(*Client).Close()` calls `no-tickets session end`; callers `defer client.Close()`
- `client.WithActor(notickets.ActorOverrides{CallID: …, PromptTokens: …}, func(c *notickets.Client) error { … })` provides scoped overrides
- Conformance test uses `net/http/httptest`

### 4. Shared conformance harness for all wrappers
status: not_started

The three wrappers' conformance tests share a contract: spawn the wrapper, run a known publish flow, assert the captured wire body matches an expected shape. Define the expected shape once (in a JSON fixture), have each wrapper's test reference it. Catches drift between wrappers.

**Files to modify/create:**
- `wrappers/shared/conformance-fixtures/agent-actor-publish.json` (new)
- `wrappers/shared/conformance-fixtures/human-actor-publish.json` (new)
- `wrappers/shared/conformance-fixtures/with-actor-override.json` (new)
- `wrappers/shared/README.md` — document the conformance protocol

**Expected changes:**
- Three fixture files, each describing input (constructor args + publish args) and expected wire body
- Each wrapper's `conformance_test.*` loads the fixtures and asserts wire-body equality (modulo per-test transport details)
- New wrapper languages added later inherit the fixture suite

### 5. Wrapper documentation: actor model section
status: not_started

Each wrapper's README gains an "Actor identity" section. Documents what's inherited from the constructor, what `withActor` overrides, and how the session is cleaned up. Includes a worked example per language.

**Files to modify/create:**
- `wrappers/typescript/README.md`
- `wrappers/python/README.md`
- `wrappers/go/README.md`

**Expected changes:**
- Each README has the same section structure: "What gets inherited (when you opt in)", "Per-call overrides", "Session cleanup", "Publishing without actor info", "Cookbook"
- Cookbook example: a CI runner spawning the wrapper with `agentId: "github-actions"` and **no `model`**, publishing a build-completed event. No `"n/a"` sentinels anywhere.
- "Publishing without actor info" section: shows the no-config constructor path and notes that the binary prints a one-time hint to stderr on first such publish

## Dependencies

- **TS / Python / Go wrapper packages**: this feature integrates with each wrapper's spawn path. TS wrapper survival is settled (ADR-0003, 2026-05-20) — it returns as `@magic-ingredients/no-tickets` with `/client` export. Python and Go wrappers ship per their own roadmap; if either slips, this feature ships partially (one or two languages at a time is fine).
- **Feature 1 (`no-tickets session` lifecycle)**: the subcommands this feature exercises must exist and be stable. Don't ship this feature before `no-tickets session` graduates from spike to documented public contract.
- **`@magic-ingredients/no-tickets-schemas` packages**: each wrapper's typed constructor surface depends on the language-native schemas package the schemas-distribution pipeline emits (TS package today; Pydantic + Go-structs roadmap items in their respective wrapper packages).
- **`docs/fixes/stream-mode.md`** (not_started): when stream mode lands, the wrapper spawn shape changes — coordinate the conformance fixtures to work over both spawn shapes (one-spawn-per-publish vs persistent-subprocess).

## Testing Strategy

### Unit Tests

- Each wrapper's session module: spawn args correctly assembled from constructor config; `no-tickets session end` called exactly once even with multiple `close()` invocations
- `withActor` override merging: per-call fields override session fields; missing per-call fields fall back to session

### Integration Tests

- Each wrapper runs the shared conformance fixtures and asserts wire-body equality
- Spawn-on-first-publish lazy behaviour (only when actor config was passed): constructor does not spawn `no-tickets`; first `publish()` does
- Cleanup on process exit: a wrapper instance with actor config that's not explicitly closed gets cleaned up by `no-tickets session end` via the exit hook
- No-actor-config path: constructor is a pure no-op for session management; `publish()` invocations carry no `metadata`; the binary's one-time hint reaches the wrapper's stderr

### Manual Testing

- Run each wrapper's example app (with actor config) against staging; verify the events land with the expected `metadata.actor`
- Run each wrapper's example app (without actor config); verify events land without `metadata` and the hint appears once on stderr
- Kill the wrapper process abruptly (SIGKILL); verify the next start still works (the session file's expiry handles the orphan)

## Implementation Notes

- The wrappers are intentionally thin. The temptation to reimplement actor resolution in each language must be resisted — every per-language reimpl is a drift surface. If `no-tickets` doesn't expose what a wrapper needs, fix `no-tickets` rather than working around in the wrapper.
- `withActor` is sugar — under the hood it's just additional `--call-id` / `--prompt-tokens` / etc. flags on the spawned `no-tickets publish` call. No magic.
- **Stream mode is in scope but tracked separately** at `docs/fixes/stream-mode.md`. When `--stream` lands, the wrapper spawn shape changes — a long-lived `no-tickets publish --stream` subprocess instead of one spawn per publish. The session lifecycle is the same (start at wrapper init, end at wrapper close) but per-call overrides flow as JSON lines over the stream subprocess's stdin, per the protocol that fix owns. Coordinate so this feature's conformance fixtures work over both spawn shapes.
- Wrappers should reject construction with invalid actor config locally (using the schemas package) rather than spawning `no-tickets` and letting it error. Fast feedback for the caller.
- Wrappers must not invent actor config from environment variables. If the caller didn't pass `agentId`, no `session start` runs, and events publish unattributed. The wrapper README's actor section says this explicitly.

## Workflow Example

TypeScript:
```ts
import { NoTickets, withActor } from '@magic-ingredients/no-tickets';

const client = new NoTickets({
  agentId: 'claude',
  model: 'claude-opus-4-7',
  provider: 'anthropic',
  thinkingEffort: 'high',
});

// Inherits the session's actor block
await client.publish('product.feature.status_changed.v1', {
  featureId: 'feat-1',
  fromStatus: 'review',
  toStatus: 'done',
});

// Per-call overrides without rewriting the session
await withActor({ callId: 'call-xyz', promptTokens: 1234 }, async () => {
  await client.publish('ai.completion.recorded.v1', { /* … */ });
});

// Process-exit hook calls no-tickets session end automatically
```

Python:
```python
from no_tickets import NoTickets, with_actor

with NoTickets(agent_id='codex', model='gpt-5') as client:
    client.publish('product.task.created.v1', {...})

    with with_actor(call_id='call-xyz', prompt_tokens=1234):
        client.publish('ai.completion.recorded.v1', {...})
```

Go (with actor):
```go
client, err := notickets.New(notickets.Config{
    AgentID: "tiny-brain",
    // Model omitted — tiny-brain isn't an LLM, no model field on the wire
})
if err != nil { /* … */ }
defer client.Close()

client.Publish("scm.commit.landed.v1", scmPayload)

client.WithActor(notickets.ActorOverrides{CallID: "call-xyz"}, func(c *notickets.Client) error {
    return c.Publish("ai.completion.recorded.v1", aiPayload)
})
```

Go (unattributed — opt-out by simply not passing config):
```go
client, err := notickets.New(notickets.Config{}) // no actor config
if err != nil { /* … */ }
defer client.Close() // no-op for session lifecycle; still releases other resources

client.Publish("scm.commit.landed.v1", scmPayload)
// → event lands without `metadata`. On first such publish in a fresh
//   <config-dir>, the binary prints the one-time hint to stderr.
```

## Benefits

- Actor model integrates into TS / Python / Go with ~zero per-language logic — the wrappers are still ~50–80 LOC
- Callers opt into attribution by passing actor config; opting out is the absence of config, not a separate API
- Drift between language bindings caught by the shared conformance fixture suite
- New wrapper languages (Ruby, Rust-native, etc.) start with the actor model already wired
- Calling code reads naturally in every language — the actor block doesn't leak into per-publish ergonomics
