---
id: migration-script
prd_id: no-tickets-client
number: 9
title: Migration Script (docs/prd/ → .notickets/)
status: not_started
created: 2026-04-17
updated: 2026-04-22
---

# Feature: Migration Script (docs/prd/ → .notickets/)

## Description

CLI command that converts existing `docs/prd/` repos (tiny-brain format) to the `.notickets/` format. Originally task 5 of Feature 24 in the service repo PRD. The migration script lives in this package since it's a general-purpose tool for any repo migrating to the no-tickets format.

## Acceptance Criteria

- [ ] `npx no-tickets migrate` scans `docs/prd/` and converts to `.notickets/`
- [ ] `npx no-tickets migrate --dry-run` shows what would change without writing
- [ ] Frontmatter fields mapped correctly (prd.md → epic.md, prd_id → epic, number removed)
- [ ] Task statuses and commit SHAs preserved
- [ ] `.notickets/config.json` created from existing preferences if present
- [ ] Non-destructive — copies to .notickets/, does not delete docs/prd/

## Tasks

### 1. Build migration command

**Files to modify/create:**
- `src/commands/migrate.ts`

**Expected changes:**
- `npx no-tickets migrate` — scans `docs/prd/`, converts to `.notickets/`
- Maps: `prd.md` → `epic.md` (update frontmatter type: epic)
- Moves: `features/*.md` → flat in epic directory (update frontmatter: prd_id → epic)
- Removes: `number` field from frontmatter
- Preserves: task statuses, commit SHAs (in meta field)
- Creates: `.notickets/config.json` from existing preferences if present
- Dry-run mode: `npx no-tickets migrate --dry-run`
- Prints summary: "Migrated X epics, Y features, Z fixes"

## Dependencies

- Feature 1 (CLI Auth) — needs CLI command infrastructure
- Feature 4 (OSS Launch) — needs package published

## Testing Strategy

### Unit Tests
- All frontmatter field mappings convert correctly
- Directory structure transforms correctly (nested features → flat)
- Dry-run produces no file writes
- Handles edge cases: missing frontmatter, empty task sections

### Integration Tests
- Migrate a real docs/prd/ repo → all data preserved in .notickets/
- Push after migration → API accepts the payload
