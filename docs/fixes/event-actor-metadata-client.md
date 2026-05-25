---
id: event-actor-metadata-client
type: fix
title: Client-side surface for the event-actor-metadata PRD
phase: development
status: completed
severity: low
created: 2026-05-21T00:00:00.000Z
updated: 2026-05-25T00:00:00.000Z
reported: 2026-05-21T00:00:00.000Z
resolved: 2026-05-25T00:00:00.000Z
resolution:
  rootCause: |
    The canonical event-actor-metadata PRD needed client-side surface
    landed in no-tickets-client: the `no-tickets session` lifecycle,
    actor resolution + metadata emission on `publish`, the public
    binary contract for the new `--actor-*` flags, the Rust
    `nt-schemas` validator wired against the metadata schema, and
    (Feature 5) the widened four-namespace metadata contract.
  fix:
    - "Implemented `no-tickets session start | show | end` with atomic
      session-file writes under `<config-dir>/active-session.json`."
    - "Added actor resolution + envelope `metadata` emission +
      first-publish hint to `no-tickets publish`."
    - "Documented the public binary contract for the new flags in
      `docs/cli-reference.md`, `docs/cookbook/actor-resolution.md`,
      and the rust-spike-notes Phase 1 follow-up section."
    - "Wired `validate_metadata` into `crates/nt-schemas/` against
      schemas v0.2.2."
    - "Bumped `crates/nt-schemas/build.rs` to schemas v0.3.0 and
      pinned the widened four-namespace metadata contract (`actor`,
      `execution`, `initiator`, `extra`) via 42 metadata parity tests."
  filesModified:
    - crates/nt-cli/src/session.rs
    - crates/nt-cli/src/state.rs
    - crates/nt-cli/src/atomic_write.rs
    - crates/nt-cli/src/commands/session.rs
    - crates/nt-cli/src/actor.rs
    - crates/nt-cli/src/hint.rs
    - crates/nt-schemas/build.rs
    - crates/nt-schemas/tests/metadata.rs
    - docs/cli-reference.md
    - docs/cookbook/actor-resolution.md
    - docs/rust-spike-notes.md
archived: true
---

# Fix: Client-side surface for the event-actor-metadata PRD

## Canonical spec

The full PRD spans both the `no-tickets-client` and `no-tickets-service`
repos. Its canonical home is the service repo (most of the work lives
there, and the schema source-of-truth lives there too):

**Canonical PRD:** https://github.com/magic-ingredients/no-tickets-service/blob/main/docs/prd/event-actor-metadata/prd.md

Adopted into the service repo at commit `1029c321` (on `staging`; merges
to `main` via the standard CI/PR flow). The PRD's previous home in
this repo (`docs/prd/event-actor-metadata/`) was removed in the same
commit that finalised this fix doc — see the git history if you need
the pre-move snapshot.

This fix tracks ONLY the client-repo work for that PRD. Server-side
tasks (schema definitions, envelope acceptance, DB column, indexes,
read APIs, UI) live in the service repo and aren't tracked here.

## Tasks

In-scope here: the client-side subset of the canonical PRD's
Feature 1 (Schemas + `no-tickets session` + publish actor wiring),
plus the client portion of Feature 4 (per-language wrappers).

### 1. Implement `no-tickets session start / show / end` subcommands
status: completed
commitSha: ddaac08

Atomic-write session file under `<config-dir>/active-session.json`
via the new `crates/nt-cli/src/session.rs` + `state.rs` +
`atomic_write.rs` modules; clap-bound subcommands in
`commands/session.rs`. Maps to Feature 1 / Task 4 of the canonical PRD.

### 2. Add actor resolution + `metadata` emission + first-publish hint to `no-tickets publish`
status: completed
commitSha: 7de873e

New `crates/nt-cli/src/actor.rs` (resolver + types) and
`hint.rs` (one-time hint decision). Envelope gains optional
`metadata` between `data` and `source`. Maps to Feature 1 / Task 5 of
the canonical PRD.

### 3. Document the public binary contract for `metadata`
status: completed
commitSha: 44865f5

New `docs/cli-reference.md` + `docs/cookbook/actor-resolution.md`;
appended Phase 1 notes to `docs/rust-spike-notes.md`. Maps to
Feature 1 / Task 6 of the canonical PRD.

### 4. Wire `validate_metadata` into the Rust `nt-schemas` crate
status: completed
commitSha: e7d0172

Maps to Feature 1 / Task 3 of the canonical PRD. Blocked on the
service repo shipping a schemas bundle that includes `metadataSchema`
as a top-level entry alongside the per-event-type schemas. Once that
lands, `crates/nt-schemas/build.rs` picks it up via the existing fetch
path; this task adds the `validate_metadata` public function + parity
fixtures alongside the existing event-type validator.

### 5. Per-language wrappers inherit `no-tickets session`
status: superseded
commitSha: null

Originally scoped against the rich-SDK shape implied by the
canonical PRD's Feature 4. Resolved 2026-05-20 (memory-recorded
decision): the TS wrapper returns as `@magic-ingredients/no-tickets`
— a ~50–80 LOC spawn shim that runs `no-tickets session start/end`
and a `withActor()` callback wrapper, not a re-implementation of
the resolver. The task body as written no longer matches the
implementation plan, and the work itself is still blocked on the
canonical PRD's Phases 2-3 (service repo) shipping. Moving the
spawn-shim work to a fresh fix once those phases unblock it keeps
the scope honest. Python / Go wrappers are out of scope unless
re-prioritised separately.

### 6. Bump Rust `nt-schemas` to v0.3.0 and pin the widened metadata contract
status: completed
commitSha: 458f681

Maps to canonical PRD event-actor-metadata Feature 5 Task 4 (Rust
parity), referenced as `implementedIn: no-tickets-client` in the
service repo. Schemas v0.3.0 widens `eventMetadataSchema` to four
optional sibling namespaces (`actor`, `execution`, `initiator`,
`extra`), makes `actor` optional on the metadata block, and adds the
`executionContextSchema`. Every v0.2.x payload still validates.

**Files to modify/create:**
- `crates/nt-schemas/build.rs` — bump `SCHEMAS_VERSION` to `"0.3.0"`
- `crates/nt-schemas/tests/metadata.rs` — flip the two v0.2.2 "actor
  required" assertions to match the widened contract; add positive
  + negative coverage for `execution`, `initiator`, `extra`, plus a
  cross-namespace shape.

**Expected changes:**
- `validate_metadata({})` now passes — empty metadata blocks are
  legal in v0.3.0.
- The "schema-non-trivially-loaded" sentinel switches its payload
  from `{}` (now valid) to something still clearly invalid
  (e.g., `{"actor": "not-an-object"}`).

## Resolution

Closed 2026-05-25. Tasks 1-4 + 6 completed; Task 5 superseded (see
its body for the rationale and where the spawn-shim work moves
next). The client-side surface for the canonical PRD is in place;
session + publish + nt-schemas parity all land in this fix, and
the wider per-language wrapper work is correctly tracked under
its own future fix once service-repo Phases 2-3 ship.

## Related

- Canonical PRD (above) — full design rationale, non-goals, and
  cross-repo task list
- [`docs/cli-reference.md`](../cli-reference.md) — public binary
  contract for the `--actor-*` flags and `session` subcommand
- [`docs/cookbook/actor-resolution.md`](../cookbook/actor-resolution.md) —
  worked examples per harness shape
- [`docs/rust-spike-notes.md`](../rust-spike-notes.md) — Phase 1
  follow-up section with Rust-side lessons from Tasks 1-2 above
- [`docs/fixes/headless-init-device-code.md`](./headless-init-device-code.md) —
  enables the human-actor branch on CI / SSH / sandbox hosts (sister fix)
