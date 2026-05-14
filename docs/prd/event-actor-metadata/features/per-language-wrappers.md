---
id: per-language-wrappers
prd_id: event-actor-metadata
number: 4
title: Per-language wrappers inherit `nt session`
status: not_started
created: 2026-05-14
updated: 2026-05-14
---

# Feature: Per-language wrappers inherit `nt session`

## Description

Phase 4 of the actor-metadata rollout. Pairs with Phase 4 of the `cross-platform-cli-binary` fix — when the per-language wrappers ship (TS / Python / Go, each ~50–80 LOC over the `nt` binary), they inherit the actor model for free by calling `nt session start` at init and `nt session end` at shutdown.

The whole point of the wrapper model is that the binary owns the integration logic. Actor resolution is one of the things that gets resolved once in `nt` rather than reimplemented per language. This feature is mostly conformance — proving each wrapper does the lifecycle handshake correctly — plus a small `withActor()` convenience for callers who want per-call overrides without poking at the session file.

Lands when the rest of `cross-platform-cli-binary` Phase 4 lands. If the wrappers slip, this feature slips with them — the actor model is fully functional through `nt` direct invocation without per-language wrappers.

## Acceptance Criteria

- [ ] TS wrapper (`@magic-ingredients/no-tickets`) spawns `nt session start` on first use and `nt session end` on process exit / explicit close
- [ ] Python wrapper does the same lifecycle dance via `subprocess.Popen` + `atexit`
- [ ] Go wrapper does the same lifecycle dance via `exec.Cmd` + deferred cleanup
- [ ] Each wrapper exposes a `withActor(overrides, fn)` API for one-off actor overrides on a single publish without rewriting the session file
- [ ] Each wrapper's conformance test: spawn nt, declare session, publish, assert the wire payload contains the expected `metadata.actor`
- [ ] Each wrapper documents the actor model in its README — what gets inherited, what can be overridden, how `nt session end` is automated
- [ ] Wrappers do NOT reimplement actor resolution. They pass values into `nt` flags or rely on the session file `nt` already manages

## Tasks

### 1. TS wrapper: spawn `nt session start` + `withActor` API
status: not_started

The TS wrapper (`@magic-ingredients/no-tickets`, ~50–80 LOC over spawn-glue) gains a session lifecycle. First call to `publish()` spawns `nt session start` with values from the wrapper's constructor (`new NoTickets({ agentId, model, … })`). Process exit hook (`process.on('beforeExit')`) spawns `nt session end`. The `withActor(overrides, fn)` helper wraps a callback so its publishes carry actor-override flags.

**Files to modify/create:**
- `wrappers/typescript/src/session.ts` (new)
- `wrappers/typescript/src/index.ts`
- `wrappers/typescript/src/with-actor.ts` (new)
- `wrappers/typescript/test/session.test.ts` (new)
- `wrappers/typescript/test/conformance.test.ts` (new)

**Expected changes:**
- Constructor accepts `{ agentId, model, provider?, thinkingEffort?, sessionId? }` and spawns `nt session start` lazily on first publish
- `process.on('beforeExit')` invokes `nt session end`
- `withActor({ callId, promptTokens, … }, async () => { await client.publish(…) })` threads override flags into the underlying `nt publish`
- Conformance test asserts the wire body (captured via wiremock) contains the expected `metadata.actor`

### 2. Python wrapper: spawn `nt session start` + `with_actor` API
status: not_started

Python equivalent of Task 1. Uses `subprocess.run` for short-lived spawns or `subprocess.Popen` for `--stream` mode. Session lifecycle hooks tie to `atexit.register` and an optional context manager (`with NoTickets(agent_id=…, model=…) as client:`).

**Files to modify/create:**
- `wrappers/python/no_tickets/session.py` (new)
- `wrappers/python/no_tickets/__init__.py`
- `wrappers/python/no_tickets/with_actor.py` (new)
- `wrappers/python/tests/test_session.py` (new)
- `wrappers/python/tests/test_conformance.py` (new)

**Expected changes:**
- `NoTickets(agent_id=…, model=…, provider=…, thinking_effort=…, session_id=…)` constructor
- Context-manager protocol calls `nt session start` on `__enter__` and `nt session end` on `__exit__`
- `with_actor({"call_id": …, "prompt_tokens": …})` as a decorator and a context manager
- Conformance test via a fake HTTP server (wiremock-equivalent in Python — `pytest-httpserver`)

### 3. Go wrapper: spawn `nt session start` + `WithActor` API
status: not_started

Go equivalent of Task 1. `os/exec.Cmd` for spawns. Session lifecycle via a `Close()` method on the client; idiomatic Go callers use `defer client.Close()`.

**Files to modify/create:**
- `wrappers/go/notickets/session.go` (new)
- `wrappers/go/notickets/client.go`
- `wrappers/go/notickets/with_actor.go` (new)
- `wrappers/go/notickets/session_test.go` (new)
- `wrappers/go/notickets/conformance_test.go` (new)

**Expected changes:**
- `notickets.New(notickets.Config{AgentID: …, Model: …, …})` constructor calls `nt session start`
- `(*Client).Close()` calls `nt session end`; callers `defer client.Close()`
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
- Each README has the same section structure: "What gets inherited", "Per-call overrides", "Session cleanup", "Cookbook"
- Cookbook example: a CI runner spawning the wrapper with `agentId: "github-actions"`, `model: "n/a"`, publishing a build-completed event

## Dependencies

- **`cross-platform-cli-binary` Phase 4 (per-language wrappers)**: this feature only exists if those wrappers exist. The wrapper-level integration of `nt session` is the value here; the wrappers themselves are out of this PRD's scope.
- **Feature 1 (`nt session` lifecycle)**: the subcommands this feature exercises must exist and be stable. Don't ship this feature before `nt session` graduates from spike to documented public contract.
- **`@magic-ingredients/no-tickets-schemas` packages**: each wrapper's typed constructor surface depends on the language-native schemas package the schemas-distribution pipeline emits (TS package today; Pydantic + Go-structs in `cross-platform-cli-binary` Phase 4).

## Testing Strategy

### Unit Tests

- Each wrapper's session module: spawn args correctly assembled from constructor config; `nt session end` called exactly once even with multiple `close()` invocations
- `withActor` override merging: per-call fields override session fields; missing per-call fields fall back to session

### Integration Tests

- Each wrapper runs the shared conformance fixtures and asserts wire-body equality
- Spawn-on-first-publish lazy behaviour: constructor does not spawn `nt`; first `publish()` does
- Cleanup on process exit: a wrapper instance that's not explicitly closed gets cleaned up by `nt session end` via the exit hook

### Manual Testing

- Run each wrapper's example app against staging; verify the events land with the expected `metadata.actor`
- Kill the wrapper process abruptly (SIGKILL); verify the next start still works (the session file's expiry handles the orphan)

## Implementation Notes

- The wrappers are intentionally thin. The temptation to reimplement actor resolution in each language must be resisted — every per-language reimpl is a drift surface. If `nt` doesn't expose what a wrapper needs, fix `nt` rather than working around in the wrapper.
- `withActor` is sugar — under the hood it's just additional `--call-id` / `--prompt-tokens` / etc. flags on the spawned `nt publish` call. No magic.
- The `--stream` mode (cross-platform-cli-binary Task 4b) significantly changes the spawn shape: instead of one `nt publish` per event, a long-lived `nt publish --stream` subprocess. The session lifecycle is the same (start at wrapper init, end at wrapper close) but per-call overrides flow on stdin JSON lines, not flags.
- Wrappers should reject construction with invalid actor config locally (using the schemas package) rather than spawning `nt` and letting it error. Fast feedback for the caller.

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

// Process-exit hook calls nt session end automatically
```

Python:
```python
from no_tickets import NoTickets, with_actor

with NoTickets(agent_id='codex', model='gpt-5') as client:
    client.publish('product.task.created.v1', {...})

    with with_actor(call_id='call-xyz', prompt_tokens=1234):
        client.publish('ai.completion.recorded.v1', {...})
```

Go:
```go
client, err := notickets.New(notickets.Config{
    AgentID: "tiny-brain",
    Model:   "n/a",
})
if err != nil { /* … */ }
defer client.Close()

client.Publish("scm.commit.landed.v1", scmPayload)

client.WithActor(notickets.ActorOverrides{CallID: "call-xyz"}, func(c *notickets.Client) error {
    return c.Publish("ai.completion.recorded.v1", aiPayload)
})
```

## Benefits

- Actor model integrates into TS / Python / Go with ~zero per-language logic — the wrappers are still ~50–80 LOC
- Drift between language bindings caught by the shared conformance fixture suite
- New wrapper languages (Ruby, Rust-native, etc.) start with the actor model already wired
- Calling code reads naturally in every language — the actor block doesn't leak into per-publish ergonomics
