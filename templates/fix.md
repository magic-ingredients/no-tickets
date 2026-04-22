---
id: your-fix-id
type: fix
epic: parent-epic-id
title: Brief description of the bug
phase: development
status: not_started
severity: medium
created: 2026-01-01
updated: 2026-01-01
reported: 2026-01-01T00:00:00.000Z
resolved: null
# When completed, add:
# resolution:
#   rootCause: Brief description of what caused the issue
#   fix:
#     - First fix action taken
#     - Second fix action taken
#   filesModified:
#     - path/to/file1.ts
#     - path/to/file2.ts
---

# Fix: Brief description of the bug

## Issue Summary

**Reported:** [Date]
**Severity:** [low | medium | high | critical]

## Reproduction Steps
1. Step to reproduce
2. Step to reproduce
3. Observe the bug

### Expected Behavior
[What should happen]

### Actual Behavior
[What actually happens]

## Root Cause

[Explain why this bug occurs. Be specific about the code path and logic error.]

### Affected Files
- `path/to/affected/file.ts`
- `path/to/another/file.ts`

## Test Plan

### Regression Tests (must pass unchanged)
| File | Cases | Status |
|------|-------|--------|
| path/to/test.ts | all existing | - |

### New Tests (to be added)
| File | Case | Status |
|------|------|--------|
| path/to/test.ts | test reproducing the bug | - |

## Tasks

### 1. Write failing test
status: not_started

Add test that reproduces the bug.

**Files to modify/create:**
- `path/to/test.ts`

**Expected changes:**
- Add test case that fails with current code
- Test should pass after fix

### 2. Implement fix
status: not_started

Fix the root cause.

**Files to modify/create:**
- `path/to/file.ts`

**Expected changes:**
- Fix the logic error
- Ensure backward compatibility

## Acceptance Criteria

- [ ] Bug no longer reproduces
- [ ] New test passes
- [ ] All regression tests pass

## Resolution

When all tasks are complete, update the YAML frontmatter:
1. Set `status: completed`
2. Set `resolved:` to ISO timestamp
3. Add `resolution:` object with `rootCause`, `fix` (array), and `filesModified` (array)

## Lessons Learned

[Optional: What can we do to prevent similar issues?]

- Lesson 1
- Lesson 2
