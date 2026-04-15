# Epic Creation Skill

## When to Use

Create an epic when the user describes:
- A new product feature or capability
- A system enhancement or improvement
- A major technical initiative
- Multi-step implementation requiring planning

## Workflow

### Step 1: Engage in Interactive Planning

Work iteratively with the user to understand:
- **Purpose**: What problem are we solving?
- **Goals**: What do we want to achieve?
- **User needs**: Who benefits and how?
- **Features**: What functionality is needed?
- **Constraints**: What limitations exist?

Ask clarifying questions. Don't jump straight to creating files.

### Step 2: Create Epic Directory

```bash
mkdir -p .notickets/{epic-id}
```

### Step 3: Create Epic File

Use the template at `templates/epic.md` and save to `.notickets/{epic-id}/epic.md`.

**YAML Frontmatter:**
```yaml
---
id: descriptive-kebab-case-id
type: epic
title: "Clear, User-Focused Title"
status: not_started
created: YYYY-MM-DD
updated: YYYY-MM-DD
---
```

### Step 4: Write Content

Add these sections:
- `# {Title}` — heading matching the frontmatter title
- Description paragraph explaining what the epic achieves
- `## Goals` — bulleted list of 2-5 concrete goals
- `## Features` — links to feature files (added as features are created)

### Step 5: Create Feature Files

For each feature identified, create a file at `.notickets/{epic-id}/{feature-id}.md`.

Use the create-feature skill or the `templates/feature.md` template.

**Task Format (Critical for Parsing):**
Tasks MUST use this format for automatic extraction:
```markdown
## Tasks

### 1. First task title
status: not_started

Description of task...

**Files to modify/create:**
- file1.ts
- file2.ts

### 2. Second task title
status: not_started

Description...
```

**Task Granularity Guidance:**
- Each task should represent a single deliverable unit of work
- Aim for 2-4 hours of work per task
- A task describes WHAT to build, not HOW
- Avoid tasks that produce no deliverable (e.g., "review code", "run tests")

### Step 6: Sync and Push

After creating files, run:
```bash
npx no-tickets push
```

This reads the `.notickets/` directory and syncs state to the dashboard.

### Step 7: Confirm Creation

Tell the user:
> "I've created epic '{title}' with {N} features at `.notickets/{epic-id}/`"

Offer to add more features using the create-feature skill.

## Quality Checklist

Before finalizing:
- [ ] All YAML frontmatter fields filled
- [ ] ID is unique and in kebab-case
- [ ] Description clearly states the problem
- [ ] Goals are concrete and measurable
- [ ] Each feature has its own markdown file
- [ ] Features listed in epic's ## Features section
- [ ] Tasks use `### N. Task Name` format with `status:` line
- [ ] Tasks describe deliverables, not process steps

## Template

- Epic: `templates/epic.md`

## Example

```
User: "We need to add user onboarding with email verification"

Claude:
1. Ask: "What should the onboarding flow include? Just email verification or also profile setup?"
2. Ask: "Should it work with corporate email filters? Any timeout requirements?"
3. Once clarified, create:
   - .notickets/user-onboarding/epic.md
   - .notickets/user-onboarding/email-verification.md
   - .notickets/user-onboarding/profile-setup.md
4. Run `npx no-tickets push`
5. Confirm: "Created epic 'User Onboarding' with 2 features"
```
