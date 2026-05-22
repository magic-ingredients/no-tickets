---
id: event-actor-metadata-client
type: fix
title: Client-side surface for the event-actor-metadata PRD
phase: development
status: in_progress
severity: low
created: 2026-05-21T00:00:00.000Z
updated: 2026-05-21T00:00:00.000Z
reported: 2026-05-21T00:00:00.000Z
resolved: null
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
status: not_started

Maps to Feature 1 / Task 3 of the canonical PRD. Blocked on the
service repo shipping a schemas bundle that includes `metadataSchema`
as a top-level entry alongside the per-event-type schemas. Once that
lands, `crates/nt-schemas/build.rs` picks it up via the existing fetch
path; this task adds the `validate_metadata` public function + parity
fixtures alongside the existing event-type validator.

### 5. Per-language wrappers inherit `no-tickets session`
status: not_started

Maps to Feature 4 of the canonical PRD (per-language wrappers). Adds
`session start` / `session end` spawning + `withActor()` helpers to
the TS / Python / Go wrappers. Tracked here once Phase 4 is unblocked
by the canonical PRD's Phases 2-3 in the service repo.

## Resolution criteria

This fix moves to `status: completed` when:

- Task 4 above lands (Rust validator parity for the metadata schema).
- Task 5 above lands (per-language wrapper integrations).

Until then, the fix stays `in_progress`. The session + publish flow
work end-to-end today against any server that accepts (or ignores)
the optional `metadata` envelope field — when the server-side PRD
phases ship, callers gain the audit-trail and UI surface they enable.

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
