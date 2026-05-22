---
id: cli-reference-generation
type: fix
title: Auto-generate CLI reference MDX from the nt-cli Clap tree
phase: development
status: in_progress
severity: low
created: 2026-05-22T00:00:00.000Z
updated: 2026-05-22T00:00:00.000Z
reported: 2026-05-22T00:00:00.000Z
resolved: null
---

# Fix: Auto-generate CLI reference MDX from the nt-cli Clap tree

## Canonical PRD

This fix tracks the client-repo work for one task of the docs-site
feature in the service repo's canonical PRD:

**PRD:** https://github.com/magic-ingredients/no-tickets-service/blob/main/docs/prd/no-tickets-team-dashboard/features/documentation-site.md
**Feature:** Documentation Site — Mintlify + Marketing Link-Out + In-App Help Button (Task 4 in that file's numbering, Task 23-9 in the dashboard PRD's flat task list).

The canonical task is now tracked here. The corresponding entry in
the canonical PRD has been marked `superseded` with an
`implementedIn: no-tickets-client` pointer.

## Problem

`docs.no-tickets.com/cli-reference/` currently has a single hand-
written `overview.mdx` and no per-command pages. Mintlify ships
nothing built-in for CLI reference generation (it has first-class
OpenAPI for HTTP APIs but no CLI equivalent). Stripe CLI, GitHub
CLI, and Cloudflare Wrangler all solve this the same way: emit MDX
from the framework's command tree as part of the release flow.

Clap is the source of truth for `nt-cli`'s command names, flags,
help text, and possible-values lists. Walking
`clap::Command::get_subcommands()` and emitting one MDX file per
command keeps the docs in lock-step with the binary's actual
surface — no hand-edited reference drifting away from `--help`.

## Tasks

### 1. Hidden `no-tickets internal generate-docs` subcommand
status: completed
commitSha: 08f10c1

Walk the Clap tree and emit one MDX file per subcommand into the
target directory. `internal` namespace + hidden flag keeps it out of
the public `--help` output; it's a build-time tool, not a user
command.

**Files to modify/create:**
- `crates/nt-cli/src/commands/internal/generate_docs.rs` (new)
- `crates/nt-cli/src/commands/internal/mod.rs` — register subcommand
- `crates/nt-cli/src/commands/mod.rs` — wire `internal` group
- `crates/nt-cli/tests/generate-docs.rs` (new) — snapshot tests

**Expected changes:**
- Each emitted file has Mintlify-compatible frontmatter (`title`,
  `description`), a `## Usage` block (synopsis line), a `## Flags`
  table (long/short/default/description), and a `## Examples` block
  populated from `#[command(after_long_help = "...")]` annotations
  on the Clap structs.
- Output path: `<target>/<command>.mdx` for top-level commands,
  `<target>/<group>/<command>.mdx` for nested.
- Idempotent: re-running against the same target directory produces
  byte-identical output (no timestamps in frontmatter).

### 2. Snapshot test pinning the emitter output
status: completed
commitSha: dd662ef

The generator's output is the wire contract between the client repo
and the docs repo. Snapshot tests fix a regression where, say, a
clap-derive macro upgrade changes the rendered flag table format
silently.

**Files to modify/create:**
- `crates/nt-cli/tests/generate-docs.rs`
- `crates/nt-cli/tests/snapshots/` — committed MDX fixtures

### 3. Release-tag workflow that syncs MDX into the docs repo
status: completed
commitSha: 098b2c8

On every release tag, run the emitter against a fresh checkout of
`no-tickets-docs` and open a PR (or push directly to a
`cli-sync-<tag>` branch).

**Files to modify/create:**
- `.github/workflows/sync-cli-docs.yml` (new)

**Expected changes:**
- Triggered by `release.published` (or a `workflow_dispatch` for
  manual back-fills).
- Uses a fine-grained PAT scoped to the docs repo so the PR isn't
  authored by `github-actions[bot]`.
- Diff-aware: if the emitted output matches what's already in the
  docs repo, the workflow exits 0 with no PR opened.

### 4. Document the generation contract in the docs runbook
status: not_started

The docs repo's runbook (`docs/runbooks/docs-site.md`) needs the
contract written down: where MDX lands, the navigation slot that
surfaces it, and the manual back-fill recipe. The client repo
shouldn't push generated MDX before the docs runbook is updated.

**Files to modify/create:**
- (no-tickets-docs repo) `docs/runbooks/docs-site.md` — coordinate
  with that repo's owner; this fix lands the client side, the docs
  runbook is a tiny follow-up there.

## Out of scope

- Hand-written narrative content for the CLI (the existing
  `cli-reference/overview.mdx` stays; this fix adds the per-command
  generated pages alongside it).
- TypeScript SDK reference. The legacy TS npm CLI is retired (see
  `cross-platform-cli-binary` fix); the Rust binary is the canonical
  surface.

## Resolution

Set `status: completed` once the release-tag workflow has run at
least once end-to-end (emitter → PR → docs repo merge) and the
Mintlify navigation surfaces the generated pages. Notify the
service-repo PRD so the canonical task's `superseded` pointer can
be tightened with the landing SHA.
