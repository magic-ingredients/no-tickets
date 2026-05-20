---
id: stream-mode
title: "no-tickets publish --stream — warm in-process publishing for per-language wrappers"
status: not_started
severity: major
reported: 2026-05-20T00:00:00.000Z
resolved: null
---

# Fix: `--stream` mode for warm in-process publishing

Extracted from `cross-platform-cli-binary` Task 27 (superseded there in
favour of standalone tracking — this is substantive feature work with
its own protocol contract, test surface, and downstream Phase 4
dependencies).

## Context

Per-event spawn cost on `no-tickets publish` is ~50 ms (binary cold
start + arg parsing + auth resolution + connection establishment).
That's fine for ad-hoc CLI use. It compounds badly when a wrapper
publishes many events from a long-running process — every
`no-tickets publish` invocation pays the 50 ms again.

A `--stream` mode keeps one subprocess alive across many publish calls
and brings per-event overhead to ~1 ms. Same pattern as `git cat-file
--batch`, `clangd`, `aspell -a` — well-trodden territory for tools
that want to handle warm state without becoming a daemon.

## Phase 4 dependency

The per-language wrappers (Python, Go, TypeScript) currently planned
for Phase 4 of the no-tickets client roadmap spawn the `no-tickets`
binary for every publish call. Without `--stream`, every wrapper
inherits the 50 ms cold-start cost on every event; with it, the wrappers
can be sub-millisecond.

That makes `--stream` a Phase 4 prerequisite. Without it the wrappers
are unattractive vs. just writing `subprocess.run(["no-tickets",
"publish", …])` once and accepting the per-event cost.

## Protocol contract

The shape was drafted in `cross-platform-cli-binary` ("Public binary
contract" section); reproduced verbatim here as the canonical source
once that fix doc archives:

```
no-tickets publish --stream [--project DEFAULT] [--token-env-var X] [--url Y]
```

**Behaviour:**

- **stdin**: one JSON object per line. Each line is
  `{ id, type, data, project?, occurredAt?, ... }` — `id` is a
  caller-chosen correlation token (any string).
- **stdout**: one JSON object per line. Each line is
  `{ id, ok: true, ingested, deduped, ids } | { id, ok: false, error: <typed-error> }`
  — `id` matches the request line.
- **stderr**: reserved for fatal binary-level errors (e.g., bad startup
  flags). Not used for per-event errors (those go on stdout with
  `ok: false`).
- **EOF on stdin**: binary drains in-flight requests, writes remaining
  responses, exits 0.
- **stdin closed mid-flight**: binary writes responses for completed
  requests, exits 0. In-progress requests get
  `ok: false, error: { error: "transport_aborted" }`.

**Multi-project per stream**: each stream request line MAY override the
default `--project` by including `project` in the JSON. The binary uses
per-line override → flag default → `NO_TICKETS_TOKEN` env. Token
resolution happens once per project per stream session (cached). This
lets a single subprocess serve many projects from one parent — useful
for orchestrators like tiny-brain.

**Cost analysis:**
- First call: ~50 ms (binary cold start)
- Subsequent calls: ~1 ms (pipe write + read)

## Wrapper expectations (informational)

These behaviours live in the per-language wrapper packages, not the
binary itself. Listed here so the binary's protocol surface matches the
real consumer needs:

- Spawn-on-first-publish; reuse for subsequent calls
- Match request id to response by an internal map (responses MAY
  arrive out-of-order)
- Handle subprocess crash by re-spawning transparently
- Kill subprocess on parent exit (POSIX process group inheritance
  handles this on most platforms; explicit `proc.kill()` on parent's
  exit handler as a safety belt)
- Parse stderr for fatal errors and `ok: false` payloads on stdout for
  per-event errors
- Translate to typed exceptions in the caller's language

## Tasks

### 1. Implement `publish --stream` subcommand
status: not_started

`crates/nt-cli/src/commands/publish_stream.rs`. Reads stdin line-by-line;
each line spawns a tokio task that does the schema-validate + HTTPS
POST and writes the response to stdout. Use a bounded mpsc channel from
worker tasks back to a single stdout-writer task so writes don't
interleave.

Token resolution: one cache per project, populated from `NO_TICKETS_TOKEN`
env or the local registry on first sighting. Cache lives for the
session.

### 2. EOF + crash semantics
status: not_started

The harder half — getting drain-on-EOF and "stdin closed mid-flight"
correct. Test surface for both:
- stdin closed cleanly → all in-flight responses written → exit 0
- stdin closed mid-publish → in-progress publishes get
  `transport_aborted` → exit 0
- network failure in flight → that one event gets `ok: false` on stdout;
  other events keep flowing
- panic in one event handler → other events keep flowing (use
  `catch_unwind` / per-task isolation)

### 3. Multi-project token-cache
status: not_started

Per-line `project` override → use the corresponding token. Cache
mapping in a `HashMap<String, ResolvedAuth>` populated lazily. A line
referencing an unregistered project gets
`ok: false, error: { error: "project_not_registered" }` without
poisoning the cache.

### 4. Protocol documentation
status: not_started

`docs/binary-stream-protocol.md` — public protocol doc. Versioned via
a `protocol_version` field on a startup handshake message (sent by
the binary as the first line of stdout when `--stream` is set):

```json
{ "protocol_version": 1, "binary_version": "0.x.y" }
```

Wrappers reject startup if `protocol_version` is a number they don't
recognise. This lets us evolve the protocol without breaking deployed
wrappers — they fail loud rather than silently mis-parse.

### 5. Benchmark + acceptance
status: not_started

`crates/nt-cli/tests/stream-mode.rs`:

- 10,000 events streamed through one subprocess in <2 s end-to-end
  (bounded by network + server, not binary overhead)
- Per-event overhead measured at <2 ms median on the wrapper side
- Crash recovery: if the binary panics mid-stream, in-flight responses
  surface as `ok: false, transport_aborted`; the wrapper can re-spawn
  cleanly

## Acceptance Criteria

- All five tasks complete + a passing integration test that exercises
  the documented behaviour
- `docs/binary-stream-protocol.md` published with examples and the
  versioning rule
- At least one consumer (the TypeScript wrapper from Phase 4) drives
  the stream end-to-end as part of its own acceptance
- Per-event overhead documented as a benchmark line in the release
  notes, so users can decide whether `--stream` is worth wiring
