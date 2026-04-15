# Feature Creation Skill

## When to Use

Add a feature when the user wants to:
- Add new functionality to an existing epic
- Break down a capability into trackable tasks
- Document implementation details for a specific piece of work

## Workflow

### Step 1: Identify Target Epic

Ask the user which epic to add the feature to, or detect from context.

List existing epics:
```bash
ls .notickets/
```

Each directory is an epic. Verify by checking for `epic.md` inside.

### Step 2: Understand the Feature

Work with the user to define:
- **What**: What does this feature do?
- **Why**: Why is it needed?
- **Acceptance criteria**: How do we know it's done?
- **Tasks**: What implementation steps are required?

### Step 3: Create Feature File

Use the template at `templates/feature.md`.

Save to: `.notickets/{epic-id}/{feature-id}.md`

**YAML Frontmatter:**
```yaml
---
id: feature-kebab-case-id
type: feature
epic: parent-epic-id
title: Feature Title
phase: ideation
status: not_started
created: YYYY-MM-DD
updated: YYYY-MM-DD
---
```

### Step 4: Define Acceptance Criteria

Add testable criteria using checkboxes:

```markdown
## Acceptance Criteria

- [ ] Users can verify their email address
- [ ] Verification link expires after 24 hours
- [ ] Resend functionality works
```

### Step 5: Define Tasks

**Critical:** Use this exact format for task extraction:

```markdown
## Tasks

### 1. First task title
status: not_started

Description of what needs to be done.

**Files to modify/create:**
- `path/to/file1.ts`
- `path/to/file2.ts`

### 2. Second task title
status: not_started

Description...
```

Each `### N. Title` becomes a trackable task.

**Task Granularity Guidance:**
- Each task should represent a single deliverable unit of work
- Aim for 2-4 hours of work per task
- A task describes WHAT to build, not HOW
- Avoid tasks that produce no deliverable (e.g., "review code", "run tests")

### Step 6: Update Parent Epic

Add a reference to the new feature in `.notickets/{epic-id}/epic.md`:

```markdown
## Features

- [feature-id.md](feature-id.md) — Brief description of the feature
```

### Step 7: Sync and Push

```bash
npx no-tickets push
```

### Step 8: Confirm Creation

Tell the user:
> "I've added feature '{title}' to epic '{epic-id}' with {N} tasks."

## Quality Checklist

- [ ] Feature ID is unique within the epic
- [ ] `epic` field matches parent epic ID exactly
- [ ] Acceptance criteria are testable (use checkboxes)
- [ ] Tasks use `### N. Task Name` format with `status:` line
- [ ] Each task has files to modify listed
- [ ] Feature is linked from parent epic
- [ ] Tasks describe deliverables, not process steps

## Template

- Feature: `templates/feature.md`

## Example

```
User: "Add email verification to the onboarding epic"

Claude:
1. Confirm epic: "Adding to user-onboarding epic?"
2. Discuss: "What happens on timeout? Should it support resend?"
3. Create: .notickets/user-onboarding/email-verification.md
4. Update: .notickets/user-onboarding/epic.md (add feature reference)
5. Run `npx no-tickets push`
6. Confirm: "Added 'Email Verification' with 4 tasks"
```
