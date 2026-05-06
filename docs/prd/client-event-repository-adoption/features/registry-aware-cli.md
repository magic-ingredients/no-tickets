---
id: registry-aware-cli
prd_id: client-event-repository-adoption
number: 4
title: nt publish / nt event / nt subject / nt action CLIs
status: not_started
created: 2026-04-27
updated: 2026-05-06
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
                   [--subject <type:id>]
                   [--source <s>]
                   [--parent <event-id>]
                   [--trace <id>]
                   [--dedupe-key <s>]

nt subject create --type <t> --external-id <id> --display-name <n> [--metadata <json>]
nt subject get <type> <id>
nt subject list --type <t> [--archived]

nt action <interaction-id> --input <json|@file|->
                           [--subject <type:id>]
```

### Registry-aware behaviours

- `nt event list` groups by domain, marks deprecated types, dims types the caller cannot write (server-side filtering already excludes most; deprecated-but-visible reads remain).
- `nt event describe` synthesises an example payload from the JSON Schema (uses field defaults, then enum first values, then placeholders).
- `nt publish <type-id>` validates `--data` against the cached JSON Schema *before* hitting the network. Validation errors print field paths.
- Unknown `<type-id>` triggers a fuzzy-match against the cached list: top-3 suggestions printed, exit non-zero, no network call.
- `--data -` reads stdin (pipe-friendly); `--data @path/to/file.json` reads a file.

### Drift notification

On any registry-aware command (any of the new verbs), after the command's primary action completes successfully, the async refresh result is checked. If the diff vs prior cache is non-empty, a single line is printed to stderr:

```
ℹ 3 new event types since last sync: engineering.incident.fired.v1, support.ticket.email_drafted.v1, ... — run `nt event list` to see them.
```

Never blocks the command. Suppressed by `--quiet` and the existing `NO_TICKETS_QUIET` env var.

## Acceptance Criteria

- [ ] `nt event list` prints types grouped by domain; `--domain` filters; `--deprecated` includes deprecated.
- [ ] `nt event describe <type-id>` prints required/optional fields, dedupe strategy, retention, and a synthesised example payload.
- [ ] `nt publish <type-id> --data <json>` validates locally then sends; `--data -` and `--data @file` work.
- [ ] Unknown type id prints fuzzy-match suggestions and exits non-zero without sending.
- [ ] `nt subject` and `nt action` round-trip against the server.
- [ ] Drift notification appears on registry diffs; suppressed by `--quiet`.
- [ ] All commands exit 0 on success, non-zero on validation failure, non-zero on server error.
- [ ] CLI help text generated lists new verbs; no reference to removed `push` command.

## Tasks

### 1. nt event list

**Files to modify/create:**
- `src/cli/commands/event/list.ts` (new)
- `src/cli/commands/event/list.test.ts` (new)
- `src/cli.ts` — register subcommand

**Expected changes:**
- Reads cache via `client.events.list({ domain, deprecated })`.
- Renders grouped output; deprecated types marked.
- Tests: empty cache fetches; populated cache reads; --domain filter; --deprecated flag.

### 2. nt event describe

**Files to modify/create:**
- `src/cli/commands/event/describe.ts` (new)
- `src/cli/commands/event/describe.test.ts` (new)
- `src/cli/lib/schema-render.ts` (new — renders JSON Schema as human form)
- `src/cli/lib/schema-render.test.ts` (new)
- `src/cli/lib/example-synth.ts` (new — synthesises example payload from JSON Schema)
- `src/cli/lib/example-synth.test.ts` (new)

**Expected changes:**
- `schema-render` walks JSON Schema producing required/optional groupings with type annotations.
- `example-synth` produces a minimal valid payload from a JSON Schema, preferring defaults → enum first values → typed placeholders.
- Tests cover representative shapes: simple object, nested, enums, arrays, refs.

### 3. nt publish <type-id>

**Files to modify/create:**
- `src/cli/commands/event/emit.ts` (new)
- `src/cli/commands/event/emit.test.ts` (new)
- `src/cli/lib/data-input.ts` (new — handles `--data <json|@file|->`)
- `src/cli/lib/data-input.test.ts` (new)
- `src/cli/lib/fuzzy-match.ts` (new)
- `src/cli/lib/fuzzy-match.test.ts` (new)

**Expected changes:**
- Reads `--data` from inline JSON, file, or stdin.
- Validates against cached JSON Schema using a JSON Schema validator (e.g. ajv).
- Unknown type → `fuzzyMatch(input, cachedTypeIds, { topN: 3 })` → suggestions printed, exit 2.
- Validation error → field path + message printed, exit 1.
- Server error → mapped error printed, exit 3.
- Tests cover each path.

### 4. nt subject

**Files to modify/create:**
- `src/cli/commands/subject/create.ts` (new)
- `src/cli/commands/subject/get.ts` (new)
- `src/cli/commands/subject/list.ts` (new)
- `src/cli/commands/subject/*.test.ts`

**Expected changes:**
- Thin wrappers over `subjects.create/get/list` from Feature 2.
- Output as JSON by default; `--format table` for human reads.
- Tests cover happy path and 404 mapping.

### 5. nt action

**Files to modify/create:**
- `src/cli/commands/action.ts` (new)
- `src/cli/commands/action.test.ts` (new)

**Expected changes:**
- Wraps `runInteraction(id, { input, subject? })`.
- `--input` follows the same `--data <json|@file|->` convention.
- Output renders the response (event ids).
- Tests cover happy path, permission denial, unknown interaction id.

### 6. Drift notification

**Files to modify/create:**
- `src/cli/lib/drift-notify.ts` (new)
- `src/cli/lib/drift-notify.test.ts` (new)
- Wire into each new command's success path

**Expected changes:**
- After a command's primary action, compare pre/post cache (refresh result from Feature 3).
- Print one stderr line listing up to 3 new type ids; truncate with `...`.
- Suppressed by `--quiet` and `NO_TICKETS_QUIET=1`.
- Tests cover: no diff → no print; small diff → full list; large diff → truncated.

### 7. CLI help + docs

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
