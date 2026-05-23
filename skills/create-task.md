# Task Creation Skill

## When to Use

Add a task when the user wants to:
- Add a new task to an existing feature or fix
- Break down a feature into more granular steps
- Track additional work discovered during implementation

## Workflow

### Step 1: Identify Target Feature

Ask the user which feature or fix to add the task to, or detect from context.

List features in an epic:
```bash
ls .notickets/{epic-id}/
```

### Step 2: Read Existing Tasks

Read the feature/fix file to find the `## Tasks` section and determine the next task number.

### Step 3: Append New Task

Add the new task at the end of the `## Tasks` section, before any section that follows (like `## Dependencies`).

**Format:**
```markdown
### {N}. {task title}
status: not_started

{description of what needs to be done}

**Files to modify/create:**
- `path/to/file.ts`
```

**Rules:**
- Task number must be the next sequential number (no gaps)
- Do NOT modify existing tasks
- A task describes WHAT to build, not HOW
- Task title should be concise and action-oriented
- Default status is always `not_started`

### Step 4: Sync and Push

```bash
no-tickets push
```

### Step 5: Confirm

Tell the user:
> "Added task {N}: '{title}' to feature '{feature-id}'"

## Task Granularity Guidance

- Each task should represent a single deliverable unit of work
- Aim for 2-4 hours of work per task
- Avoid tasks that produce no deliverable (e.g., "review code", "run tests")

## Quality Checklist

- [ ] Task number is sequential (no gaps, no duplicates)
- [ ] Status is `not_started`
- [ ] Title is concise and action-oriented
- [ ] Description explains what needs to be done
- [ ] Files to modify are listed
- [ ] Tasks describe deliverables, not process steps

## Example

```
User: "Add a task to the email verification feature for rate limiting"

Claude:
1. Read .notickets/user-onboarding/email-verification.md
2. Find last task number (e.g., 4)
3. Append:
   ### 5. Add rate limiting to verification endpoint
   status: not_started

   Limit verification attempts to 5 per hour per email address.

   **Files to modify/create:**
   - `src/routes/verify.ts`
   - `src/middleware/rate-limit.ts`
4. Run `no-tickets push`
5. Confirm: "Added task 5: 'Add rate limiting' to email-verification"
```
