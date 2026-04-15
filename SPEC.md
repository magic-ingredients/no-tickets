# no-tickets Format Specification v1

## Overview

The no-tickets format defines how work is structured in a repository using markdown files with YAML frontmatter. Any tool that reads this format can compute project state — epics, features, tasks, and progress.

## Directory Structure

```
.notickets/
├── config.json              # Connection config (gitignored)
├── {epic-slug}/             # One directory per epic
│   ├── epic.md              # Epic definition
│   ├── {feature-slug}.md    # Feature files
│   └── {fix-slug}.md        # Fix/bug files
└── {another-epic}/
    └── ...
```

## Document Types

### Epic

A body of work with a goal. Contains features and fixes.

**Frontmatter:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | yes | Unique kebab-case identifier |
| `type` | `"epic"` | yes | Document type |
| `title` | string | yes | Human-readable title |
| `status` | EntityStatus | yes | Current status |
| `created` | string | yes | ISO date (YYYY-MM-DD) |
| `updated` | string | yes | ISO date (YYYY-MM-DD) |
| `meta` | object | no | Extensible metadata |

**Sections:** Description, Goals (bulleted list), Features (links to files)

### Feature

A shippable unit of functionality within an epic.

**Frontmatter:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | yes | Unique kebab-case identifier |
| `type` | `"feature"` | yes | Document type |
| `epic` | string | yes | Parent epic ID |
| `title` | string | yes | Human-readable title |
| `phase` | Phase | yes | Current delivery phase |
| `status` | EntityStatus | yes | Current status |
| `assignee` | string | no | Human or agent name |
| `assignee_type` | AssigneeType | no | `"human"` or `"agent"` |
| `created` | string | yes | ISO date (YYYY-MM-DD) |
| `updated` | string | yes | ISO date (YYYY-MM-DD) |
| `meta` | object | no | Extensible metadata |

**Sections:** Description, Acceptance Criteria (checkbox list), Tasks (numbered), Dependencies

### Fix

A bug fix within an epic. Same structure as Feature with additional fields.

**Frontmatter:** Same as Feature, plus:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | `"fix"` | yes | Document type |
| `severity` | Severity | no | `"critical"`, `"high"`, `"medium"`, `"low"` |

**Sections:** Description, Reproduction Steps, Tasks (numbered), Root Cause

## Enums

### Phase

The delivery phase of a feature or fix:

| Value | Description |
|-------|-------------|
| `ideation` | Planning and design |
| `development` | Implementation in progress |
| `testing` | Tests and validation |
| `review` | Code review / PR open |
| `done` | Merged and shipped |

### EntityStatus

| Value | Description |
|-------|-------------|
| `not_started` | No work begun |
| `in_progress` | Work underway |
| `completed` | All tasks done |

### TaskStatus

| Value | Description |
|-------|-------------|
| `not_started` | Task not begun |
| `in_progress` | Task underway |
| `completed` | Task done |

### AssigneeType

| Value | Description |
|-------|-------------|
| `human` | Human developer |
| `agent` | AI coding agent |

## Task List Format

Features and fixes contain a `## Tasks` section with numbered headings:

```markdown
## Tasks

### 1. Task title
status: not_started

Description of the task.

### 2. Another task
status: completed

Description.
```

**Parsing rules:**
- Tasks are identified by `### N. Title` headings (N is a positive integer)
- The `status:` line must follow the heading (first non-empty line after)
- Valid status values: `not_started`, `in_progress`, `completed`
- Task numbers must be sequential starting from 1
- No duplicate task numbers

## config.json

Connection configuration for syncing to no-tickets.com:

```json
{
  "teamId": "team-abc",
  "projectId": "proj-xyz",
  "apiUrl": "https://api.no-tickets.com",
  "formatVersion": 1
}
```

This file should be gitignored — it contains team-specific configuration.

## Meta Field

The `meta` field in frontmatter is an extensible object for tool-specific data. Any key-value pairs are allowed. Examples:

```yaml
meta:
  quality_score: 82
  quality_grade: B
  tdd_compliant: true
  pr_url: https://github.com/org/repo/pull/47
```

Tools should read only the keys they understand and pass through unknown keys.

## Validation Rules

1. All `id` fields must be kebab-case (lowercase letters, numbers, hyphens)
2. All date fields must be YYYY-MM-DD format
3. Feature/fix `epic` field must reference an existing epic directory
4. Task numbers must be sequential (1, 2, 3...) with no gaps or duplicates
5. All enum values must be from the defined set
6. IDs must be unique across all documents in the repository

## Format Version

The format version is tracked in `config.json` as `formatVersion`. Current version: **1**.

Future versions will maintain backward compatibility where possible. Breaking changes will increment the major version.
