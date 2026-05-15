

## tiny-brain - start
---
version: 0.23.3
---

## Repository Context

Before starting work, read `AGENTS.md` for comprehensive project context (tech stack, commands, structure).
Also read `.tiny-brain/analysis.json` for detailed detection data and test patterns.

## Commit Message Format

### CRITICAL: Commit Header Requirements

**BEFORE EVERY COMMIT**, check if you're working on tracked work:

1. **Active PRDs?** Check `.tiny-brain/progress/` for `in_progress` status
2. **Open Fixes?** Check `docs/fixes/*.md` frontmatter for `not_started` or `in_progress` status

**If YES, you MUST include tracking headers or the commit will be REJECTED.**

### For PRD-tracked work:
```
feat(scope): description

PRD: {prd-id}
Feature: {feature-id}
Task: {exact task description from progress.json}

Description of changes...
```

### For Fix-tracked work:
```
feat(scope): description

Fix: {fix-id}
Task: {exact task description from fix document}

Description of changes...
```

### Commit Types
| Type | When | Headers Required? |
|------|------|-------------------|
| `test:` | Writing failing tests (TDD RED) | Yes |
| `feat:` | Implementation (TDD GREEN) | Yes |
| `fix:` | Bug fixes | Yes |
| `refactor:` | Code improvement | Yes |
| `chore:` | Maintenance (untracked) | No |
| `untracked:` | Work not related to any PRD or Fix | No |

**WARNING:** The commit-msg hook will reject commits missing required headers.

### Updating Markdown After Commits

**For PRD tasks:**
1. Open the feature file: `docs/prd/{prd-id}/features/{feature-id}.md`
2. Update the task with status and commitSha:
   ```markdown
   ### 1. Task description
   status: completed
   commitSha: abc1234
   ```
3. Run: `npx tiny-brain task sync docs/prd/{prd-id}/features/{feature-id}.md`
4. If ALL tasks in the feature are complete, update the PRD status

### Fix Status Workflow

Fix documents have four statuses: `not_started` → `in_progress` → `completed` | `superseded`

**When starting work on a fix:**
1. Open the fix file: `docs/fixes/{fix-id}.md`
2. Update frontmatter: `status: in_progress`
3. Run: `npx tiny-brain task sync docs/fixes/{fix-id}.md`

**After each commit:**
1. Update the completed task(s) in the markdown:
   ```markdown
   ### 1. Task description
   status: completed
   commitSha: abc1234
   ```
2. If one commit addresses multiple tasks, use the same commitSha for all of them
3. If a task is no longer needed (work done elsewhere or obsolete), mark it superseded:
   ```markdown
   ### 3. Obsolete task
   status: superseded
   commitSha: null
   ```
4. Run: `npx tiny-brain task sync docs/fixes/{fix-id}.md`

**When all tasks are complete:**
1. **ONLY set `status: completed`** when ALL tasks are accounted for:
   - 100% of tasks must have either `status: completed` (with commitSha) or `status: superseded`
   - Example: A fix with 5 tasks could be: 3 completed + 2 superseded = completed
   - A fix with incomplete tasks stays `in_progress`
2. Update YAML frontmatter:
   - Set `status: completed`
   - Set `resolved: YYYY-MM-DDTHH:mm:ss.sssZ` (ISO timestamp)
   - Add `resolution` object:
     ```yaml
     resolution:
       rootCause: Brief description of what caused the issue
       fix:
         - First fix action taken
         - Second fix action taken
       filesModified:
         - path/to/file1.ts
         - path/to/file2.ts
     ```
3. Run: `npx tiny-brain task sync docs/fixes/{fix-id}.md`

**Note:** The markdown file is the source of truth. The `task sync` command updates `progress.json` from the markdown.

## Starting Work on a Task (MANDATORY)

**As soon as you know which task you're about to work on**, call `pipeline` to enter the pipeline BEFORE doing anything else.

```bash
# For PRD-tracked work (start in RED — write tests first):
npx tiny-brain pipeline --task-id 'Exact task description' --prd my-prd --feature my-feature --agent red

# For Fix-tracked work (start in RED):
npx tiny-brain pipeline --task-id 'Exact task description' --fix my-fix-id --agent red

# For tasks that don't need tests (config, shell scripts, docs, templates):
npx tiny-brain pipeline --task-id 'Exact task description' --fix my-fix-id --agent green
```

`--agent red` enters at the RED phase (write tests first). `--agent green` skips RED and enters at GREEN (for non-testable changes).

**After calling pipeline, tell the user:**

```
🧠 🔴 Red phase started for: [task description]
# or with --agent green:
🧠 🟢 Green phase started for: [task description] (RED skipped)
```

This applies to ALL work — not just coding. If you're reading code to understand a task, designing a solution, or rewriting a spec, you are working on the task and should call `pipeline` first.

## TDD Workflow

IMPORTANT: This repository follows strict Test-Driven Development (TDD) with a 3-phase commit workflow.

**Red → Green → Refactor Cycle:**

1. **Red Phase** (`test:` or `test(scope):` commits):
   - **Enter the pipeline first** (see "Starting Work on a Task" above). Call `pipeline --agent red` if not done.
   - Write failing tests first
   - Tests SHOULD fail (that's the point!)
   - Use: `git commit -m "test: ..."` or `git commit -m "test(api): ..."`
   - Git hook runs typecheck + lint but SKIPS tests
   - Post-commit hook calls `pipeline` to advance

2. **Green Phase** (`feat:` or `feat(scope):` commits):
   - Implement minimum code to make tests pass
   - Use: `git commit -m "feat: ..."` or `git commit -m "feat(auth): ..."`
   - Git hook runs full checks (typecheck + lint + test)
   - Post-commit hook calls `pipeline` which starts the review pipeline
   - Pipeline outputs a system-reminder telling you which review agent to spawn

3. **Review Pipeline** (triggered by pipeline system-reminder after `feat:` commit):
   - The configured `reviewPipeline` determines which reviews run and in what order
   - Review agent analyses the work, calls `persist` to save findings, then calls `pipeline` to advance
   - Pipeline outputs instructions — follow them exactly
   - If refactoring needed → commit with `refactor:` prefix
   - Post-commit hook calls `pipeline` which advances to the next review (or complete)

4. **Progress Commit** (end of cycle):
   - After the full TDD cycle is complete
   - Run: `npx tiny-brain commit progress`

### What you call vs what hooks/agents call

| Command | Who calls it | When |
|---------|-------------|------|
| `pipeline --agent red` | **YOU** | Before starting work (enters pipeline) |
| `pipeline --agent green` | **YOU** | Before starting work on non-testable tasks (skips red) |
| `commit progress` | **YOU** | After the full TDD cycle completes |
| `pipeline --agent <step> --sha <sha>` | Post-commit hook (automatic) | After every tracked commit |
| `persist <agent>-review --sha <sha>` | Review agent (automatic) | Saves review JSON |
| `pipeline --agent <type> --decision <verdict>` | Review agent (automatic) | Advances pipeline after review |
| `pipeline --quiet` | Commit-msg hook (automatic) | Validates refactor commits |

**You NEVER call** `commit track`, `persist`, or `pipeline --decision` directly. The hooks and agents handle them.

## Operational Tracking Directory (.tiny-brain/)

The `.tiny-brain/` directory stores operational tracking data separate from documentation:

```
.tiny-brain/
├── analysis.json       # Detected tech stack and repo analysis
└── tech/               # Technology-specific context files
    ├── config.json     # Tech context mode configuration
    └── {name}.md       # One file per detected technology
```

**Key distinction:**
- **Documentation** (in `docs/`) - PRD markdown, feature specs, fix analysis - permanent, reviewed
- **Operational tracking** - progress files live under `.git/tiny-brain/` (per-clone, never committed)


## tiny-brain - end


