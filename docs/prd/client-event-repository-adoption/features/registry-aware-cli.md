---
id: registry-aware-cli
prd_id: client-event-repository-adoption
number: 4
title: nt publish / nt event / nt subject / nt action CLIs
status: in_progress
created: 2026-04-27
updated: 2026-05-17
---

# Feature: nt publish / nt event / nt subject / nt action CLIs

## Description

Four new CLI verbs that wrap Features 2 and 3: `nt publish` for sending events (single or batch), `nt event` for discovery (`list` + `describe`), `nt subject` for subject promotion + lookup, `nt action` for interactions. The CLI is registry-aware throughout — `list`, `describe`, fuzzy-match suggestions, JSON Schema validation against the cache before sending.

This replaces `nt push` from a user's perspective. The legacy `nt push` was already removed in Feature 2; this feature is the user-facing replacement. `nt publish` mirrors the SDK's `publish(events: Event[])` and the wire-format `POST /v1/events` array body — single event = one CLI invocation, batch = `--batch <file.jsonl>`.

### Command surface

```
nt event list [--domain <name>] [--deprecated]
nt event describe <type-id>

nt publish <type-id> --data <json|@file|->
                   [--subject-type <t> --subject-id <id>]
                   [--source-name <name>]
                   [--source-attribute key=value]...
                   [--parent <event-id>]
                   [--trace <id>]
                   [--dedupe-key <s>]
nt publish --batch <file.jsonl>
                   [--source-name <name>]
                   [--source-attribute key=value]...

nt subject create --type <t> --external-id <id> --display-name <n> [--metadata <json>]
nt subject get <type> <id>
nt subject list --type <t> [--archived]

nt action <interaction-id> --input <json|@file|->
                           [--subject-type <t> --subject-id <id>]
```

### Subject reference encoding

Subject references are passed via separate `--subject-type <t>` and `--subject-id <id>` flags. Earlier drafts proposed `--subject <type:id>` colon-encoded, which fails for IDs containing colons (URNs, ARNs). Split flags avoid encoding gymnastics entirely.

### Source overrides at the CLI

The CLI auto-fills `source` to `{ name: 'cli', sdkVersion, version: <cli-version> }`. Callers can override:
- `--source-name <name>` overrides `source.name`
- `--source-attribute key=value` (repeatable) adds entries to `source.attributes`

Caller-supplied fields merge with auto-detected — caller wins on conflicts. Most users never need these flags; they exist for integrations invoking `nt publish` from a wrapper script that wants to identify itself.

### Registry-aware behaviours

- `nt event list` groups by domain, marks deprecated types, dims types the caller cannot write (server-side filtering already excludes most; deprecated-but-visible reads remain).
- `nt event describe` synthesises an example payload from the JSON Schema (uses field defaults, then enum first values, then placeholders) via the shared `example-synth` lib (also consumed by Feature 5's MCP `describe_event_type`).
- `nt publish <type-id>` validates `--data` against the cached JSON Schema *before* hitting the network. Validation errors print field paths. **Best-effort** — a stale cache may pass a payload the server rejects; the server is the source of truth.
- Unknown `<type-id>` triggers a fuzzy-match against the cached list: top-3 suggestions printed, exit non-zero, no network call.
- `--data -` reads stdin (pipe-friendly); `--data @path/to/file.json` reads a file. The `@` prefix is unambiguous: no valid JSON value starts with `@`.
- `--batch <file.jsonl>` reads one JSON event per line, validates each locally, sends the entire batch in a single `POST /v1/events`. Stdin batch via `nt publish --batch -`.

### Drift notification

On any registry-aware command (any of the new verbs), after the command's primary action completes successfully, the async refresh result is checked. If the diff vs prior cache is non-empty, a single line is printed to stderr:

```
ℹ 3 new event types since last sync: engineering.incident.fired.v1, support.ticket.email_drafted.v1, ... — run `nt event list` to see them.
```

Never blocks the command. Suppressed by `--quiet` and the existing `NO_TICKETS_QUIET` env var.

## Acceptance Criteria

- [ ] `nt event list` prints types grouped by domain; `--domain` filters; `--deprecated` includes deprecated.
- [ ] `nt event describe <type-id>` prints required/optional fields, dedupe strategy, retention, and a synthesised example payload (via shared `example-synth` lib).
- [ ] `nt publish <type-id> --data <json>` validates locally (best-effort) then sends; `--data -` and `--data @file` work.
- [ ] `nt publish --batch <file.jsonl>` validates and publishes every line as one event in a single request; `--batch -` reads from stdin.
- [ ] Subject references use split `--subject-type <t> --subject-id <id>` flags; no colon-encoded form.
- [ ] Source overrides via `--source-name` and repeatable `--source-attribute key=value`; merged with auto-detected source.
- [ ] Unknown type id prints fuzzy-match suggestions and exits non-zero without sending.
- [ ] `nt subject create/get/list` and `nt action` round-trip against the server.
- [ ] Drift notification appears on registry diffs (after bounded refresh wait, default 200ms); suppressed by `--quiet` and `NO_TICKETS_QUIET=1`.
- [ ] All commands exit 0 on success, non-zero on validation failure, non-zero on server error.
- [ ] CLI help text generated lists new verbs; no reference to removed `push` command.

## Tasks

### 1. nt event list

status: not_started

**Reconciliation (2026-05-17):** Genuinely not done — and not strictly required. The MCP server has `list_event_types` for the agent-discovery path; terminal-side users currently rely on `docs/install.md` and `nt --help`. Adding `nt event list` to the Rust CLI is straightforward (the registry HTTP client and cache already exist in `crates/nt-mcp/src/registry_cache.rs` — needs extracting to `nt-core` or being called from nt-cli). Defer until terminal-user demand surfaces.

**Files to modify/create:**
- `src/cli/commands/event/list.ts` (new)
- `src/cli/commands/event/list.test.ts` (new)
- `src/cli.ts` — register subcommand

**Expected changes:**
- Reads cache via `client.events.list({ domain, deprecated })`.
- Renders grouped output; deprecated types marked.
- Tests: empty cache fetches; populated cache reads; --domain filter; --deprecated flag.

### 2. nt event describe

status: not_started

**Reconciliation (2026-05-17):** Same shape as Task 1 — MCP `describe_event_type` covers the agent flow; terminal users have no equivalent yet. Defer until demand surfaces.

**Files to modify/create:**
- `src/cli/commands/event/describe.ts` (new)
- `src/cli/commands/event/describe.test.ts` (new)
- `src/cli/lib/schema-render.ts` (new — renders JSON Schema as human form)
- `src/cli/lib/schema-render.test.ts` (new)
- `src/lib/example-synth.ts` (new — synthesises example payload from JSON Schema; **shared** lib used by both this command and Feature 5's MCP `describe_event_type`)
- `src/lib/example-synth.test.ts` (new)

**Expected changes:**
- `schema-render` walks JSON Schema producing required/optional groupings with type annotations.
- `example-synth` produces a minimal valid payload from a JSON Schema, preferring defaults → enum first values → typed placeholders. Lives outside `cli/` so MCP can import it.
- Tests cover representative shapes: simple object, nested, enums, arrays, refs.

### 3. nt publish <type-id> (single event)

status: completed
commitSha: 4844b43

**Reconciliation (2026-05-17):** Landed in Rust under fix `cross-platform-cli-binary` (the TS CLI was retired in fix Task 12). `nt publish --type <id> --data <json> --project <p>` works against staging; lives at `crates/nt-cli/src/commands/publish.rs` + `publish/envelope.rs` + `publish/post.rs`. Bundled JSON Schema validation gates the call (best-effort, server authoritative). Structured error contract on stderr per fix Task 26 (commit `2bc103b`). Subject flags work; source flags work; data-from-stdin works. **Not yet implemented:** fuzzy-match suggestions on unknown type id — captured as a follow-up if user demand surfaces.

**Files to modify/create:**
- `src/cli/commands/publish/single.ts` (new)
- `src/cli/commands/publish/single.test.ts` (new)
- `src/cli/lib/data-input.ts` (new — handles `--data <json|@file|->`)
- `src/cli/lib/data-input.test.ts` (new)
- `src/cli/lib/fuzzy-match.ts` (new)
- `src/cli/lib/fuzzy-match.test.ts` (new)
- `src/cli/lib/source-flags.ts` (new — parses `--source-name` and repeatable `--source-attribute key=value`)
- `src/cli/lib/source-flags.test.ts` (new)

**Expected changes:**
- Reads `--data` from inline JSON, file, or stdin (`-`).
- Subject from `--subject-type` + `--subject-id` if both provided; otherwise undefined.
- Source from `--source-name` + repeated `--source-attribute key=value`; merged with auto-detected (caller wins).
- Validates against cached JSON Schema using a JSON Schema validator (e.g. ajv). **Best-effort** — server is authoritative; document this in the help text.
- Unknown type → `fuzzyMatch(input, cachedTypeIds, { topN: 3 })` → suggestions printed, exit 2.
- Validation error → field path + message printed, exit 1.
- Server error → mapped error printed, exit 3.
- Tests cover each path.

### 4. nt publish --batch <file.jsonl>

status: completed
commitSha: 8c8dc00

**Reconciliation (2026-05-17):** Landed in Rust as `nt publish --file <path>` (or `--file -` for stdin). Code at `crates/nt-cli/src/commands/publish_batch.rs` + `publish_batch/{jsonl,envelope,source}.rs`. Per-line source override, machine-attribute opt-in, per-line validation with batch-index propagation on server 422 — all implemented. Flag name diverges from PRD's original `--batch` to `--file`; current shape matches the cargo-dist binary surface and is documented in `docs/install.md`.

**Files to modify/create:**
- `src/cli/commands/publish/batch.ts` (new)
- `src/cli/commands/publish/batch.test.ts` (new)
- `src/cli/lib/jsonl.ts` (new — line-by-line JSON reader for files and stdin)
- `src/cli/lib/jsonl.test.ts` (new)

**Expected changes:**
- Reads JSONL from file path or stdin (`--batch -`).
- Each line is one event; parse error on any line → exit 1 with line number.
- Source flags apply to *every* event in the batch (merged with each event's own source if present).
- Validates each event against cached JSON Schema before sending; on validation failure, exit 1 with line number + field path.
- Sends all events as one `POST /v1/events`; server-side per-event 422 carries `batchIndex` which maps back to JSONL line number for diagnostics.
- Tests cover: file read, stdin read, partial-batch local validation failure, server-side per-event failure with line-number mapping.

### 5. nt subject

status: superseded
commitSha: null

**Reconciliation (2026-05-17):** Superseded per `[[project_no_subjects_in_model]]` — no production subject types are registered server-side, so a `nt subject` CLI verb has no callers. The TS `subjects.create/get/list` exists as forward-compat infrastructure (publish-client Task 3); the CLI wrapper around it is deferred indefinitely.

**Files to modify/create:**
- `src/cli/commands/subject/create.ts` (new)
- `src/cli/commands/subject/get.ts` (new)
- `src/cli/commands/subject/list.ts` (new)
- `src/cli/commands/subject/*.test.ts`

**Expected changes:**
- Thin wrappers over `subjects.create/get/list` from Feature 2.
- Output as JSON by default; `--format table` for human reads.
- Tests cover happy path and 404 mapping.

### 6. nt action

status: superseded
commitSha: null

**Reconciliation (2026-05-17):** Superseded per `[[project_workflow_by_events]]` — workflows are modelled as event sequences sharing a `run_id` with autonomous workers emitting their own events, not synchronous `run_interaction` invocations. `nt action` would wrap a primitive that's not how the system works post-2026-05-15.

**Files to modify/create:**
- `src/cli/commands/action.ts` (new)
- `src/cli/commands/action.test.ts` (new)

**Expected changes:**
- Wraps `runInteraction(id, { input, subject? })`.
- `--input` follows the same `--data <json|@file|->` convention.
- Subject from split `--subject-type` + `--subject-id` flags.
- Output renders the response (event ids).
- Tests cover happy path, permission denial, unknown interaction id.

### 7. Drift notification

status: not_started

**Reconciliation (2026-05-17):** Aspirational; not done. The mechanism requires `nt event list` / `nt event describe` (tasks 1, 2 above) to be useful, since drift notification describes which event types are new. Re-evaluate once tasks 1+2 land.

**Files to modify/create:**
- `src/cli/lib/drift-notify.ts` (new)
- `src/cli/lib/drift-notify.test.ts` (new)
- Wire into each new command's success path

**Expected changes:**
- After a command's primary action, await the async refresh promise with bounded timeout (default 200ms via `awaitRefresh` from Feature 3 Task 4).
- If refresh completed with a diff, compare pre/post cache.
- Print one stderr line listing up to 3 new type ids; truncate with `...`.
- If refresh did not complete in time, no notification this invocation; the next invocation picks up the diff.
- Suppressed by `--quiet` and `NO_TICKETS_QUIET=1`.
- Tests cover: no diff → no print; small diff → full list; large diff → truncated; refresh-too-slow → no print.

### 8. CLI help + docs

status: not_started

**Reconciliation (2026-05-17):** Partial — Rust clap auto-generates `nt --help` listing current verbs (no push references). README + `docs/install.md` have a quickstart for `nt init` / `nt publish` / `nt status` / `nt validate` per fix `cross-platform-cli-binary` Task 13 (commit `3f2c3bd`). What's *not* covered: `nt event list` / `nt event describe` quickstart — because those commands don't exist (see tasks 1, 2). Marking `not_started` until 1+2 ship and the docs gain the corresponding lines.

**Files to modify/create:**
- `src/cli.ts`
- `src/cli/help.ts` (or wherever help text lives)
- `README.md`

**Expected changes:**
- Help lists new verbs; removes any push references.
- README updated with quickstart for `nt event list` / `describe` / emit.
- Tests assert help string contains the new verbs and not `push`.

## Dependencies

- Feature 1 (envelope schemas).
- Feature 2 (transport client).
- Feature 3 (registry introspection + cache).

## Testing Strategy

### Unit Tests
- Each command's success and failure paths against a stubbed transport.
- Schema renderer and example synthesiser against representative JSON Schemas.
- Fuzzy-match correctness (Levenshtein-or-similar; cover edge cases like empty list, all-match, no-match).

### Integration Tests
- End-to-end against a fixture server: list → describe → emit happy path.
- Offline-with-cache works for list/describe; emit fails with clear diagnostic when offline.
- `--data -` from stdin; `--data @file` from file; both round-trip.
