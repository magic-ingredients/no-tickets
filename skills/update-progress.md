# Update Progress Skill

## When to Use

Update progress when:
- A task has been started or completed
- A feature should move to the next phase
- An assignee needs to be set or changed
- Work has been done that should be reflected on the board

## Workflow

### Step 1: Identify the File

Find the feature or fix file to update:
```bash
ls .notickets/{epic-id}/
```

### Step 2: Update Task Status

Open the file and change the task's status line:

```markdown
### 3. Implement the endpoint
status: completed    # was: in_progress
```

**Valid status values:**
| Status | When to Use |
|--------|-------------|
| `not_started` | Task not begun |
| `in_progress` | Work has started |
| `completed` | Task is done |

### Step 3: Update Feature Phase (if appropriate)

If the task completion changes the feature's delivery phase, update the `phase` field in the YAML frontmatter:

```yaml
phase: development    # was: ideation
```

**Phase progression:**

| From | To | When |
|------|-----|------|
| `ideation` | `development` | First task started |
| `development` | `testing` | Implementation tasks done, testing begins |
| `testing` | `review` | Ready for review / PR opened |
| `review` | `done` | Approved and shipped |

### Step 4: Update Assignee (if needed)

Set the assignee in frontmatter if not already set:

```yaml
assignee: Claude
assignee_type: agent
```

- Set `assignee_type` to `agent` if an AI tool is doing the work
- Set `assignee_type` to `human` otherwise

### Step 5: Update Dates

Always update the `updated` field in frontmatter:

```yaml
updated: 2026-04-05    # today's date
```

### Step 6: Sync and Push

```bash
npx no-tickets push
```

## Rules

- Only change task statuses — never modify task titles or descriptions
- Only advance phases forward, never backward (except `review` → `development` for rework)
- Update `updated` date whenever making changes
- If all tasks are completed, update feature `status` to `completed`

## Quality Checklist

- [ ] Task status is a valid value
- [ ] Phase progression makes sense
- [ ] `updated` date is current
- [ ] Assignee is set if work has started
- [ ] Feature status updated if all tasks done
