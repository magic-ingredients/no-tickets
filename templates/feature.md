---
id: your-feature-id
type: feature
epic: parent-epic-id
title: Your Feature Title
phase: ideation
status: not_started
created: 2026-01-01
updated: 2026-01-01
---

# Your Feature Title

## Description

[Provide a comprehensive description of what this feature does, why it's needed, and how it fits into the larger epic. Be specific about the functionality it provides.]

## Acceptance Criteria

- [ ] Criterion 1: [Specific, testable requirement]
- [ ] Criterion 2: [Specific, testable requirement]
- [ ] Criterion 3: [Specific, testable requirement]

## Test Plan

### Regression Tests (must pass unchanged)
| File | Cases | Status |
|------|-------|--------|
| path/to/test.ts | all existing | - |

### Amended Tests (expectations will change)
| File | Case | Change | Status |
|------|------|--------|--------|
| path/to/test.ts | specific case | Description of change | - |

### New Tests (to be added)
| File | Case | Status |
|------|------|--------|
| path/to/test.ts | new case for feature | - |

## Tasks

[Each task is a discrete unit of work. Describe WHAT to build — the TDD red/green/refactor cycle is HOW you build it. Do not split TDD phases into separate tasks.]

### 1. Task name
status: not_started

[Brief description of what needs to be done]

**Files to modify/create:**
- `path/to/file1.ts`
- `path/to/file2.ts`

**Expected changes:**
- Change 1: [Description]
- Change 2: [Description]

### 2. Task name
status: not_started

[Brief description of what needs to be done]

**Files to modify/create:**
- `path/to/file1.ts`

**Expected changes:**
- Change 1: [Description]

## Dependencies

- **Feature/System**: [Description of dependency]

## Testing Strategy

### Unit Tests
- Test scenario 1
- Test scenario 2

### Integration Tests
- Integration scenario 1

## Implementation Notes

[Optional: Technical notes, considerations, or edge cases for implementers]

- Note 1: [Important consideration]
- Note 2: [Technical detail]
