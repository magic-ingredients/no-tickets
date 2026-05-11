---
id: tinybrain-pipeline-upgrades
type: fix
title: tiny-brain TDD pipeline upgrades — test-adversarial phase, multi-language mutation, task-scoped diff
phase: development
status: not_started
severity: medium
created: 2026-05-11
updated: 2026-05-11
reported: 2026-05-11T00:00:00.000Z
resolved: null
---

# Fix: tiny-brain TDD pipeline upgrades

> **Scope note.** This fix tracks changes to the `tiny-brain` CLI/skill/agent
> bundle, not to the `no-tickets` repo. It lives in `no-tickets/.tiny-brain/fixes/`
> for planning convenience; the implementing commits land in the tiny-brain
> source tree. Move this file once the tiny-brain repo gets its own fixes/.

## Issue Summary

**Reported:** 2026-05-11
**Severity:** medium

Three orthogonal weaknesses in the current TDD pipeline came out of the
cross-platform-cli-binary Task 1 spike (commits 38defa7 → 026ca57, 2026-05-11):

1. **No first-class "review of tests as a specification" step.** The
   adversarial reviewer fired multiple times on `test:` commits via
   system-reminder loops — useful but unframed. Each pass caught real
   spec gaps (strict-equality vs substring inconsistency, missing
   credentials-shape coverage, NO_TICKETS_HOME-vs-HOME parity gap,
   partial-pair message format pinning, profile-error path
   non-distinction). Pre-impl test quality is currently catch-as-catch-can.

2. **Mutation review is single-language (Stryker / TS only).** The same
   spike landed Rust code; the mutation-reviewer agent had to *improvise*
   — recognise that Stryker doesn't apply, install `cargo-mutants`
   manually, and run it ad-hoc. The pivot worked but was fragile, undocumented,
   and would not repeat reliably across repos or commits.

3. **Pipeline analyses "the last commit" but tasks span multiple commits.**
   The Task 1 spike took 8 commits (RED → refine RED → refine RED → GREEN →
   refactor → refactor → docs → metadata). Each review step ran against
   `--sha <last>` only. Earlier-in-the-task commits got reviewed only
   because reviewer agents independently re-read the prior history. Robust
   review needs the *task's full diff*, not the latest commit's diff.

These three are independent but compound: an AI-driven TDD pipeline
that doesn't review tests *as a spec*, that's blind to anything beyond
TS, and that only sees the latest commit will systematically miss real
problems.

## Concrete evidence (cross-platform-cli-binary Task 1 spike, 2026-05-11)

| Pipeline-shaped gap | What actually happened |
|---|---|
| Test-quality not formally gated | Three RED iterations needed before the test suite was an acceptable spec. The current pipeline allowed each iteration only because the adversarial reviewer fires on every commit anyway — not because there's a designed phase for it. |
| Stryker doesn't speak Rust | Mutation-reviewer agent detected the language mismatch and installed `cargo-mutants` on the fly. Output was a clean 19/1/4 (killed/equivalent-survivor/unviable) but the path to that result was un-prescribed. A less capable agent would have skipped the mutation step entirely or tried to run Stryker against Rust source and produced nonsense. |
| Last-commit-only diff hides multi-commit tasks | The mutation review on commit 80f18ea correctly scoped its run to that commit's changed files, but a task that spans test commits *then* impl commits has the impl files only in the GREEN sha — running mutation against the test-commit sha would mutate empty files. Two-phase review needs task-scoped diff, not commit-scoped. |
| Known-equivalent mutants resurface every run | `urls.rs:134 && → ||` is logically unreachable (guarded by line 125's `!=` branch). The survivor will re-appear on every mutation run forever. There's currently no way to acknowledge "verified equivalent". |
| Per-language thresholds don't exist | A single `min_kill_rate` across all languages averages out regressions — e.g., a Rust regression hidden by stable TS coverage. |
| Full vs incremental scope is conflated | cargo-mutants on this single command took ~3 min for 24 mutants. Scaling to a full CLI port (~15 files) means 30+ min per commit. No way to say "fast scoped sweep per commit, full sweep nightly". |

## Design

### 1. New `adversarial-tests` reviewer agent + pipeline phase

Split the existing single `adversarial` step into two roles with
different prompt templates targeting the same reviewer agent type:

| Step | Reads | Asks |
|---|---|---|
| `adversarial-tests` | Test files (+ reference impl if one exists) | "Are these tests a complete and unambiguous specification?" |
| `adversarial-impl` | Implementation files | "Does the impl satisfy that spec with sound engineering, including bugs the tests can't catch?" |

New pipeline phase ordering:

```
RED → adversarial-tests → (refine RED if dirty, loop) →
GREEN → adversarial-impl → (refactor if dirty, loop) →
       mutation          → (refactor / add tests if dirty, loop) →
done
```

Mutation stays post-GREEN. The user's earlier proposal to put mutation
before GREEN was reconsidered: mutation testing mutates the
implementation, so it cannot run before there is an implementation.
The framing of "mutation is a test-quality check" is correct; the
ordering remains post-GREEN because of the tool's mechanics.

**Optional pre-GREEN step for ports:** `differential` — wrap the
new test suite in a shim, run it against the reference implementation,
verify pass-or-documented-divergence. Strictly stronger than
adversarial-tests alone because it grounds the spec in observed
behaviour, not agent inference. Skipped automatically when no
reference impl is configured for the task.

#### Prompt templates

`adversarial-tests` template instructs the reviewer to:
- Read the test files in the task's diff.
- If a reference implementation path is configured, read that too.
- Report: gaps in scenario coverage, brittle assertions, redundant
  tests that *don't* distinguish regressions, error-path coverage,
  cross-product coverage, position-agnostic flag handling, and any
  test whose passing admits trivial / wrong implementations.
- Verdict: clean or dirty. Dirty → user refines RED tests.

`adversarial-impl` template instructs the reviewer to:
- Read the implementation files in the task's diff.
- Assume tests are spec-grade (the prior phase gated this).
- Report: bugs the tests cannot catch (e.g., map ordering, panic
  paths, concurrency hazards), code smells, dependency choices,
  panic-on-broken-pipe and similar production hazards.
- Verdict: clean or dirty. Dirty → user refactors.

The two templates ship as prompt files in the tiny-brain bundle; the
review agent loads the right one per phase.

### 2. Multi-language mutation review

#### Adapter contract

```rust
trait MutationAdapter {
    fn name(&self) -> &'static str;
    fn language(&self) -> &'static str;
    fn detect(&self, repo: &Repo) -> AdapterAvailability;
    fn run(&self, ctx: RunContext) -> Result<MutationReport, AdapterError>;
}

enum AdapterAvailability {
    Available { tool_version: String },
    NeedsInstall { install_hint: String },
    Unsupported { reason: String },
}

struct RunContext {
    sha_range: (Sha, Sha),         // task-scoped — see section 3
    changed_files: Vec<PathBuf>,   // pre-scoped to this adapter's language
    scope_mode: ScopeMode,
    output_dir: PathBuf,
    allowlist: Vec<AllowedSurvivor>,
}

enum ScopeMode {
    ChangedFilesOnly,   // delta — per-commit / per-task
    Package,            // mid — package or crate
    Workspace,          // full — nightly CI
}

struct MutationReport {
    tool: String,
    tool_version: String,
    language: String,
    files_mutated: Vec<PathBuf>,
    totals: Totals,
    survivors: Vec<Survivor>,
    elapsed_ms: u64,
}

struct Totals {
    total: u32,
    killed: u32,
    survived: u32,
    unviable: u32,
    timeout: u32,
    skipped: u32,
}

struct Survivor {
    file: PathBuf,
    line: u32,
    column: Option<u32>,
    mutator_kind: String,
    description: String,
    diff_hunk: Option<String>,
    raw_log_path: Option<PathBuf>,
}
```

Each adapter writes structured JSON to stdout, logs to stderr only
(same gotcha as rmcp). Exits 0 on success, non-zero on tool failure.

#### Built-in adapters to ship

| Language | Tool | Output to parse | Notes |
|---|---|---|---|
| TypeScript / JavaScript | Stryker | `reports/mutation/mutation.json` | Existing repos with `stryker.config.mjs` already configured |
| Rust | `cargo-mutants` | `mutants.out/outcomes.json` | Validated against this exact tool today; 24 mutants ran cleanly |
| Python | `mutmut` | `mutmut json` stdout | Or `cosmic-ray` as alternative |
| Go | `gremlins` | `--output json` stdout | Or `go-mutesting` as alternative |
| JVM | Pitest | `target/pit-reports/mutations.xml` | XML parse |

#### Configuration format

Per-repo `.tiny-brain/mutators.toml`:

```toml
[mutators.rust]
tool = "cargo-mutants"
scope = "changed-files"
min_kill_rate = 0.85

[mutators.typescript]
tool = "stryker"
config = "stryker.config.mjs"
scope = "changed-files"
min_kill_rate = 0.80

[mutators.python]
tool = "mutmut"
scope = "changed-files"
min_kill_rate = 0.75

[scope_overrides]
nightly = "workspace"      # used when invoked with --nightly
```

Per-user/global defaults at `~/.tiny-brain/mutators.toml`.

#### Per-file language routing

Driver algorithm:

1. Compute the task's diff (section 3).
2. Walk changed paths; classify each by extension/path → language.
3. Group by language.
4. For each language present:
   a. Look up the configured adapter from `mutators.toml`.
   b. Check availability via `adapter.detect()`.
   c. If unavailable, either prompt for install or fail with a
      structured "no mutator for this language" verdict — never
      silently skip.
   d. Run the adapter against that language's file group.
5. Aggregate per-language reports; check each against its threshold.
6. Persist combined JSON report to `.tiny-brain/reviews/mutation/<task-sha>.json`.

#### Known-equivalent-mutant allowlist

`.tiny-brain/mutation-allowlist.toml`:

```toml
[[allowed_survivors]]
file = "crates/nt-cli/src/urls.rs"
line = 134
column = 16
mutator_kind = "replace_binop_and_with_or"
reason = "Unreachable: api_set != auth_set guard on line 125 makes && and || logically equivalent here"
verified_sha = "80f18ea"
verified_by = "andy"
verified_at = "2026-05-11"
```

Driver subtracts these from the active-survivor count before threshold
check. The `verified_sha` field acts as a freshness check: if the file
at the listed line has changed since `verified_sha`, the entry is
invalidated and the survivor counts again — refactoring forces
re-verification.

#### Full vs delta modes

Three explicit scope modes:

- **`changed-files`** (default for interactive pipeline) — mutate only
  files in the task's diff. Fast (~1–3 min per task). Catches
  regressions in code being actively worked on. The common case.

- **`package`** (CI on touched packages) — mutate the whole package
  (Rust crate / npm package / Python module) any file in the task's
  diff belongs to. Mid-weight (~5–15 min). Catches "this change leaks
  into adjacent code".

- **`workspace`** (nightly / pre-release) — mutate the whole codebase
  per adapter. Heavyweight (30+ min). Tightens kill-rate threshold;
  produces the canonical mutation health number.

Driver flag: `--scope changed-files|package|workspace`. Per-mutator
default in `mutators.toml` overrides global default. CI calls can set
`--nightly` which triggers the `scope_overrides.nightly` value.

### 3. Task-scoped diff

#### The bug

Current `pipeline ... --sha <sha>` passes a single commit sha to every
review step. The reviewer (and any adapter) sees only that one commit's
diff. A multi-commit task — common in TDD where RED and GREEN are
separate commits, and adversarial-impl loops can add refactor: commits
— ends up with each step reviewing a fragment of the task's actual
delta.

In today's spike, this was masked because:
- The adversarial reviewer agent voluntarily re-read prior commits to
  understand context (working as designed, but undocumented).
- The mutation review happened on the GREEN sha which already contained
  all impl files in their final state.

But for the proposed pipeline (adversarial-tests after RED, then
adversarial-impl after GREEN), the test files exist in RED-shas and the
impl files exist in GREEN-shas. A `--sha <last-commit>` lookup at
adversarial-impl time would miss the test files unless the reviewer
voluntarily looks them up. That's not robust.

#### The fix

Track task starts and pipeline scope diffs by *task range*, not by
single commit:

1. When `pipeline --agent red` is called for a task, record the current
   HEAD sha as `task_start_sha` in `<git-common-dir>/tiny-brain/tasks/<task-id>.json`.
2. When subsequent `pipeline` calls happen for the same task, compute
   the task diff as `git diff <task_start_sha>..HEAD --name-only` rather
   than `git diff HEAD~1..HEAD`.
3. Pass the full task diff to every review step and every adapter.
4. When the task closes (`commit progress` or task `status: completed`),
   archive the `task_start_sha` for later reference.

Edge cases:
- **`commit --amend`** rewrites HEAD; the `task_start_sha` still points
  at the pre-amend ancestor and the diff calculation remains correct.
- **`git rebase`** moves the task_start_sha out from under us. Mitigate:
  on every `pipeline` call, verify `task_start_sha` is reachable from
  HEAD. If not, prompt the user to re-enter the pipeline at the
  appropriate phase (cheap, manual recovery).
- **Multiple tasks open simultaneously** (interleaved work): each task
  has its own `task_start_sha`; concurrent tasks don't collide.

### 4. Per-language thresholds

Each adapter declares a kill-rate threshold in `mutators.toml`. Default
floor: 0.80. Tasks fail mutation review if *any* language's kill rate
drops below its threshold. Per-language counts are reported separately
in the aggregated JSON — no averaging across languages.

## Tasks

### 1. Define `adversarial-tests` and `adversarial-impl` prompt templates
status: not_started

Ship two prompt-template markdown files in the tiny-brain bundle (or
equivalent location depending on packaging):
- `adversarial-tests.md` — "review tests as a specification"
- `adversarial-impl.md` — "review impl assuming spec-grade tests"

Each template includes: the reviewer's reading list (test files vs impl
files), the questions to answer, the verdict criteria (clean / dirty),
the structured JSON output schema.

**Files to modify/create:**
- `prompts/adversarial-tests.md` (new)
- `prompts/adversarial-impl.md` (new)
- existing single `adversarial.md` retired or aliased to `adversarial-impl.md`

**Acceptance:**
- Two distinct prompts, each ≤ ~1 KB so they fit cleanly in the
  reviewer's context.
- Test-template explicitly tells the reviewer NOT to read impl files
  when an impl exists (to avoid the reviewer mentally collapsing both
  reviews into one).

### 2. Add `adversarial-tests` to the pipeline phase sequence
status: not_started

Insert the new phase between `red` and `green` in the default
pipeline. Make the phase ordering explicit and configurable per fix /
PRD type rather than hard-coded.

**Files to modify/create:**
- pipeline phase config schema
- default phase sequence: `["red", "adversarial-tests", "green", "adversarial-impl", "mutation"]`
- `pipeline --agent adversarial-tests` accepts a `--decision` flag like
  the existing reviewers.

**Acceptance:**
- A `test:` commit triggers an `adversarial-tests` phase.
- A `feat:`/`fix:` commit triggers `adversarial-impl` then `mutation`.
- A `dirty` verdict at `adversarial-tests` loops back to RED, not
  forward to GREEN.

### 3. Wire pipeline triggers + system-reminders for the new phase
status: not_started

Post-commit hook recognises `test:` commits and outputs an
`adversarial-tests` reminder. Stops the current "REFACTORING REQUIRED"
reminder from firing on `test:` commits when refactor was not actually
required — the current behaviour fires that reminder ahead of the
review.

**Files to modify/create:**
- post-commit hook
- system-reminder text templates

**Acceptance:**
- A `test:` commit on a tracked task emits exactly one reminder
  instructing the user to invoke the adversarial-tests agent.
- Mistake protection: if the user commits `feat:` while still in RED
  phase, the commit-msg hook rejects (already does this; verify it
  still works with the new phase).

### 4. Optional `differential` step for port tasks
status: not_started

Allow the per-fix config to declare a reference-implementation path
(e.g., `reference: "src/cli.ts"`). When present, run a `differential`
step after RED: spawn the new test suite, point it at a shim that
proxies to the reference impl, verify pass or documented divergence.

**Files to modify/create:**
- fix doc frontmatter: optional `reference: <path>` field
- differential-runner: invoked between RED and adversarial-tests

**Acceptance:**
- For a port task with a reference impl, divergent tests fail
  differential before adversarial-tests runs. Tests must be either
  fixed to match TS or annotated with a deliberate-divergence marker
  that the differential runner respects.
- For non-port tasks, the step is skipped automatically.

### 5. Define `MutationAdapter` trait + registry
status: not_started

Implement the trait shape from the Design section. Registry is a map
from language to adapter binary; per-repo `mutators.toml` overrides
defaults.

**Files to modify/create:**
- adapter trait definition
- registry loader (reads built-in defaults + per-repo + per-user TOML)
- adapter discovery / detection logic

**Acceptance:**
- Registry can return the right adapter for any of {rust, typescript,
  python, go, jvm} from default config.
- A repo-level `mutators.toml` overrides defaults.
- Unknown language → AdapterAvailability::Unsupported with a clear
  message.

### 6. Stryker adapter (TypeScript / JavaScript)
status: not_started

Wrap the existing Stryker config; parse `reports/mutation/mutation.json`
into the common `MutationReport` schema.

**Files to modify/create:**
- `adapters/stryker.ts` (or Rust binary that invokes Stryker)
- output parser for Stryker's JSON reporter

**Acceptance:**
- Running on the no-tickets repo TS code produces a valid
  MutationReport JSON.
- Tool absence emits `AdapterAvailability::NeedsInstall` with
  `npm install --save-dev @stryker-mutator/core @stryker-mutator/vitest-runner`
  (or equivalent) as the install hint.

### 7. cargo-mutants adapter (Rust)
status: not_started

Wrap `cargo-mutants`; parse `mutants.out/outcomes.json` into the common
schema. This is the proof-of-concept — today's spike already produced
this output by hand; the adapter encodes that process.

**Files to modify/create:**
- `adapters/cargo-mutants.rs` (or whatever language tiny-brain itself ships in)
- output parser for `mutants.out/outcomes.json`

**Acceptance:**
- Running on the nt-cli crate reproduces today's manual result: 24
  total, 19 killed, 1 survived, 4 unviable.
- Per-mutant `raw_log_path` field points at `mutants.out/log/<file>_line_<n>_col_<m>.log`.
- Tool absence emits `cargo install cargo-mutants` as install hint.

### 8. mutmut adapter (Python)
status: not_started

Wrap `mutmut`; parse `mutmut json` stdout.

**Files to modify/create:**
- `adapters/mutmut.py` (or equivalent)
- output parser

**Acceptance:**
- Running on a Python sample repo produces valid MutationReport JSON.
- Tool absence emits `pip install mutmut` install hint.

### 9. gremlins adapter (Go)
status: not_started

Wrap `gremlins`; parse `--output json` stdout.

**Files to modify/create:**
- `adapters/gremlins.go` (or equivalent)
- output parser

**Acceptance:**
- Running on a Go sample repo produces valid MutationReport JSON.
- Tool absence emits `go install github.com/go-gremlins/gremlins/cmd/gremlins@latest` install hint.

### 10. Per-file language routing in the driver
status: not_started

Driver consumes the task diff (Task 13), classifies each path by
language, groups, and dispatches per-language adapter calls in
parallel (adapters are independent).

**Files to modify/create:**
- driver: language classifier
- driver: per-language parallel dispatch
- driver: result aggregator

**Acceptance:**
- Commit touching both `src/cli.ts` and `crates/nt-cli/src/auth.rs`
  triggers both Stryker (on the TS file) and cargo-mutants (on the
  Rust file), aggregated into one combined report.

### 11. Known-equivalent-mutant allowlist
status: not_started

`.tiny-brain/mutation-allowlist.toml` format + driver integration.

**Files to modify/create:**
- allowlist parser
- driver: subtract active allowlist entries from survivor count
- allowlist freshness check: hash file content at the listed line
  against `verified_sha`; invalidate stale entries

**Acceptance:**
- Adding today's `urls.rs:134 && → ||` survivor to the allowlist makes
  subsequent runs report 0 active survivors instead of 1.
- Modifying that file invalidates the entry; mutation review fails
  until the entry is re-verified or removed.

### 12. Full / package / changed-files scope modes
status: not_started

Three explicit scope modes per the Design section; CLI flag
`--scope changed-files|package|workspace`; per-mutator defaults in
`mutators.toml`.

**Files to modify/create:**
- driver: scope resolution
- each adapter: scope handling

**Acceptance:**
- `pipeline ... --scope changed-files` runs ~1–3 min on a typical
  small commit.
- `pipeline ... --scope workspace --nightly` runs the full sweep with
  the nightly thresholds applied.

### 13. Task-scoped diff (the bug fix)
status: not_started

Record `task_start_sha` when `pipeline --agent red` (or `green` for
non-testable tasks) is first called for a task. All subsequent review
and adapter calls receive `git diff <task_start_sha>..HEAD` rather than
the last-commit diff.

**Files to modify/create:**
- task-state store: `<git-common-dir>/tiny-brain/tasks/<task-id>.json`
- pipeline driver: emit task-scoped diff in RunContext
- adapter contract: accept task diff, not single-commit diff
- rebase-resilience check on every pipeline call

**Acceptance:**
- A task with RED commit A and GREEN commit B has both A and B's diffs
  seen by adversarial-impl and mutation.
- `commit --amend` of B preserves task-scoped diff correctness.
- `git rebase` invalidating `task_start_sha` produces a clear
  recovery prompt, not a silent diff drift.

### 14. Per-language kill-rate thresholds + aggregation
status: not_started

Implement per-language threshold enforcement; aggregated report does
NOT average across languages.

**Files to modify/create:**
- driver: threshold enforcement per language
- report schema: per-language breakout

**Acceptance:**
- A commit dropping Rust kill-rate to 0.60 while TS stays at 0.95
  fails the mutation review (Rust threshold violated) rather than
  passing on the average.

### 15. Documentation update
status: not_started

Update tiny-brain's CLAUDE.md / docs:
- The new phase ordering and what each phase does.
- How to register a custom mutator.
- How to add an entry to `mutation-allowlist.toml`.
- The scope-mode trade-offs.
- The task-scoped-diff model and how to recover from rebase.

**Files to modify/create:**
- tiny-brain's CLAUDE.md template (the one injected into consumer repos)
- tiny-brain's user-facing docs

**Acceptance:**
- A new consumer repo gets a CLAUDE.md that describes the four-phase
  pipeline accurately.
- Per-language mutation setup is one paragraph + a config snippet.

## Test Plan

### Unit tests (per adapter)

Each adapter ships with fixture JSON output from its real tool, plus
assertions that the parser produces the expected MutationReport.

### Integration tests

- Run cargo-mutants adapter against this repo's `crates/nt-cli/` and
  assert the killed/survived/unviable counts match today's manual run.
- Run Stryker adapter against the existing TS test suite; assert it
  produces a coherent kill rate (existing Stryker config must yield
  one; this is a regression check).

### End-to-end test

- Synthetic multi-language repo with TS + Rust + Python directories
  containing one trivial function + one trivial test each. Touch all
  three in one commit. Run `pipeline --agent mutation`. Assert:
  - All three adapters invoked.
  - Combined report has three language sections.
  - Per-language threshold check works.

### Task-scoped diff regression

- Synthetic task with RED commit (test only) + GREEN commit (impl only).
- Run `pipeline --agent adversarial-impl` after GREEN; assert the
  reviewer's context shows BOTH commits' files, not just GREEN's.

## Dependencies & Coordination

- **The adapter for cargo-mutants is the highest-priority adapter**
  because it's the one with proven need (no-tickets is mid-port). Build
  it first, validate against today's manual run, then generalise to the
  other languages.
- **The task-scoped diff fix is independent** of the multi-language
  work and unblocks both the new test-adversarial phase AND existing
  multi-commit tasks. Land it first.
- **Allowlist UX matters.** The allowlist needs an ergonomic way to
  add entries (probably a `tiny-brain mutation allow --sha <s> --file
  <p> --line <n> --reason "..."` subcommand) — burying it in TOML
  hand-edits will produce stale entries.

## Lessons / Open Questions

- **Differential testing for non-port tasks?** Probably not. A
  greenfield feature has no oracle. Differential should remain a
  port-only opt-in.
- **Should mutation review run on `refactor:` commits?** Probably yes —
  refactors can subtly weaken test coverage by deleting a branch that
  used to be exercised. But it's slower; gate on adapter scope mode.
- **Cross-cutting reviews vs per-language reviews?** A commit touching
  TS and Rust gets two adapter runs but one adversarial-impl review.
  The reviewer reads files from both languages. That works for the
  adversarial step (a single reasoning agent can hold both contexts);
  it does NOT work for mutation (each tool is language-specific). The
  asymmetry is documented and intentional.
- **Survivor allowlist drift.** A survivor's `verified_sha` should
  arguably bind to the *function* containing the line, not the line
  number — line numbers move with edits. Function-name binding needs
  Tree-sitter or similar. Defer to a follow-up; line-binding is
  good enough for v1.
- **Pipeline observability.** With more phases, users will want a
  status command: "what phase is task X in?". Already partially
  supported via `progress.json`; formalise as `tiny-brain pipeline
  status --task-id <id>`.

## Acceptance Criteria

- [ ] Two distinct prompt templates ship: adversarial-tests, adversarial-impl
- [ ] Pipeline phase sequence reorders to: red → adversarial-tests → green → adversarial-impl → mutation
- [ ] `MutationAdapter` trait + registry exists; adapters for TS / Rust / Python / Go all functional
- [ ] Per-language thresholds enforced separately (no averaging)
- [ ] Allowlist mechanism for known-equivalent mutants with `verified_sha` freshness check
- [ ] Scope modes: changed-files / package / workspace selectable per run
- [ ] Task-scoped diff: review steps and adapters see the full task delta, not just last-commit
- [ ] Documentation updated; CLAUDE.md template reflects new phase ordering
- [ ] End-to-end test: synthetic multi-language repo passes mutation review with all three adapters invoked
- [ ] Backward compatibility: existing single-language Stryker users see no behavioural regression
