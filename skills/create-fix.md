# Fix Creation Skill

## When to Use

Create a fix document when:
- User reports a bug to investigate
- You identify an issue that needs tracking
- A fix requires multiple steps and test validation
- You want to document root cause analysis

## Workflow

### Step 1: Investigate the Issue

Before documenting, investigate:
- **Reproduction steps**: How to trigger the bug?
- **Expected behavior**: What should happen?
- **Actual behavior**: What actually happens?
- **Root cause**: Why is this happening?

Use exploration tools (grep, read, etc.) to understand the issue.

### Step 2: Identify Target Epic

Fixes belong to an epic. Identify which epic the bug relates to:
```bash
ls .notickets/
```

If no epic exists for this area, create one first with the create-epic skill.

### Step 3: Create Fix Document

Use the template at `templates/fix.md`.

Save to: `.notickets/{epic-id}/{fix-id}.md`

**File naming:** Use descriptive kebab-case prefixed with `fix-`:
- `fix-login-timeout.md`
- `fix-email-not-sending.md`
- `fix-progress-bar-overflow.md`

**YAML Frontmatter:**
```yaml
---
id: fix-kebab-case-id
type: fix
epic: parent-epic-id
title: Brief Description of the Bug
phase: development
status: not_started
severity: medium
created: YYYY-MM-DD
updated: YYYY-MM-DD
---
```

**Severity guide:**
- `critical` — System down, data loss, security vulnerability
- `high` — Major feature broken, no workaround
- `medium` — Feature partially broken, workaround exists
- `low` — Cosmetic issue, minor inconvenience

### Step 4: Document Root Cause

In the fix document, clearly explain:
1. What the bug is
2. What causes it (root cause)
3. What the fix approach is

**IMPORTANT:** Do NOT use `### N.` numbered headings outside the `## Tasks` section. The parser treats `### N. Title` as task definitions. Use **bold text** or unnumbered `###` headings instead.

### Step 5: Document Test Plan

**IMPORTANT:** Before documenting, analyze the codebase to identify relevant tests. Do NOT guess — read the actual test files.

1. **Find test files** for affected code
2. **Read them** to understand existing coverage
3. **Categorize:**
   - **Regression** — Tests that should continue to pass unchanged
   - **Amended** — Tests whose expectations need updating
   - **New** — Tests that need to be written

### Step 6: Define Tasks

```markdown
## Tasks

### 1. Investigate and reproduce
status: not_started

Confirm the bug and identify root cause.

### 2. Implement the fix
status: not_started

Fix the root cause.

**Files to modify:**
- `src/service.ts`
```

### Step 7: Sync and Push

```bash
npx no-tickets push
```

### Step 8: Confirm and Offer Implementation

Tell the user:
> "I've created fix document '{title}' at `.notickets/{epic-id}/{fix-id}.md` with {N} tasks."

Then **always ask**:
> "Would you like me to implement this fix now?"

## Quality Checklist

- [ ] Root cause is clearly documented
- [ ] Reproduction steps are included
- [ ] Severity is appropriate
- [ ] Tasks describe deliverables, not process steps
- [ ] No `### N.` headings outside Tasks section
- [ ] Status reflects current state

## Template

- Fix: `templates/fix.md`

## Example

```
User: "The login times out on slow connections"

Claude:
1. Investigate: Check auth timeout value, network handling
2. Identify: "Auth timeout is 5s, too short for 3G"
3. Create: .notickets/user-onboarding/fix-login-timeout.md
4. Document root cause, test plan, fix tasks
5. Run `npx no-tickets push`
6. Confirm: "Created fix document with 2 tasks"
7. Ask: "Would you like me to implement this fix now?"
```
