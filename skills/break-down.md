# Break Down Skill

## When to Use

Decompose a goal when the user describes:
- A new product or feature at a high level
- A system they want to build
- A problem they want to solve
- Multi-step work that needs structure

This is the "power skill" — it creates a complete epic structure from a natural language description.

## Workflow

### Step 1: Engage in Interactive Planning

Work iteratively with the user to understand:
- **Purpose**: What problem are we solving?
- **Goals**: What do we want to achieve?
- **Scope**: What's in and what's out?
- **Constraints**: What limitations exist?

Ask clarifying questions. Don't jump straight to creating files.

### Step 2: Design the Structure

Break the work into:
- **1 Epic** — the overall body of work
- **2-6 Features** — each a shippable unit of functionality
- **2-6 Tasks per Feature** — each a deliverable unit of work, 2-4 hours

### Step 3: Create Everything

1. Create epic directory: `mkdir -p .notickets/{epic-id}`
2. Create `epic.md` with goals from the conversation
3. Create one `.md` file per feature with:
   - Clear description
   - 2-5 acceptance criteria
   - 2-6 tasks per feature

Use the templates in `templates/` for the correct format.

**YAML Frontmatter for Epic:**
```yaml
---
id: epic-slug
type: epic
title: Epic Title
status: not_started
created: YYYY-MM-DD
updated: YYYY-MM-DD
---
```

**YAML Frontmatter for each Feature:**
```yaml
---
id: feature-slug
type: feature
epic: epic-slug
title: Feature Title
phase: ideation
status: not_started
created: YYYY-MM-DD
updated: YYYY-MM-DD
---
```

### Step 4: Task Format

**Critical — tasks MUST use this exact format:**

```markdown
## Tasks

### 1. Task title
status: not_started

Description of what needs to be done.

**Files to modify/create:**
- `path/to/file.ts`

### 2. Another task
status: not_started

Description...
```

**Task Granularity:**
- Each task should represent a single deliverable unit of work
- Aim for 2-4 hours of work per task
- A task describes WHAT to build, not HOW
- Avoid tasks that produce no deliverable (e.g., "review code", "run tests")

### Step 5: Sync and Push

```bash
no-tickets push
```

### Step 6: Confirm

Tell the user:
> "Created epic '{title}' with {N} features and {M} total tasks at `.notickets/{epic-id}/`"

List the features with task counts.

## Quality Checklist

- [ ] Epic has clear goals (2-5 bullet points)
- [ ] Features are independently shippable
- [ ] Each feature has acceptance criteria
- [ ] Tasks use `### N. Task Name` format with `status:` line
- [ ] Tasks describe deliverables, not process steps
- [ ] Total tasks give a realistic picture of the work
- [ ] All IDs are kebab-case

## Guidelines

- Features should be independently shippable — avoid features that depend on each other
- Name features by what they deliver, not by technical component
- Keep the total number of features manageable (2-6)
- If a feature has more than 6 tasks, consider splitting it into two features
- Start with the feature that has the fewest dependencies

## Example

```
User: "Build a payment system with Stripe"

Claude:
1. Ask: "What payment methods? Subscriptions or one-time? Invoicing?"
2. Once clarified, create:

.notickets/payment-system/
├── epic.md                    # Goals: accept payments, manage subscriptions
├── stripe-integration.md      # 4 tasks
├── checkout-flow.md           # 3 tasks
├── webhook-handling.md        # 3 tasks
└── invoice-generation.md      # 3 tasks

3. Run `no-tickets push`
4. Confirm: "Created epic 'Payment System' with 4 features and 13 total tasks"
```
