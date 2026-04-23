---
id: pnpm-swap
prd_id: pnpm-migration
number: 1
title: Replace npm with pnpm
status: completed
created: 2026-04-15
updated: 2026-04-15
---

# Feature: Replace npm with pnpm

## Description

Swap the package manager from npm to pnpm. This involves generating a pnpm lockfile, removing the npm lockfile, adding the `packageManager` field to package.json for corepack enforcement, and updating documentation.

## Acceptance Criteria

- [ ] `pnpm-lock.yaml` exists and is committed
- [ ] `package-lock.json` is deleted
- [ ] `package.json` has `packageManager` field with latest stable pnpm version
- [ ] README references updated from `npx` to `pnpm dlx` / `pnpm` where appropriate
- [ ] `.gitignore` unchanged (node_modules already ignored)
- [ ] All existing commands work: `pnpm run build`, `pnpm run lint`, `pnpm run test`

## Tasks

### 1. Add pnpm lockfile and packageManager field
status: completed
commitSha: 42b413f

Generate pnpm-lock.yaml, add `packageManager` field to package.json, delete package-lock.json.

**Files to modify/create:**
- `package.json`
- `pnpm-lock.yaml` (generated)

**Expected changes:**
- Add `"packageManager": "pnpm@9.x.x"` to package.json
- Remove `package-lock.json`
- Generate `pnpm-lock.yaml` via `pnpm import` then `pnpm install`

### 2. Update README and documentation for pnpm
status: completed
commitSha: 09ee6de

Update all npm/npx references in README.md to use pnpm equivalents.

**Files to modify/create:**
- `README.md`

**Expected changes:**
- Replace `npx no-tickets` with `pnpm dlx no-tickets` or `pnpm exec` where appropriate
- Update install instructions

## Dependencies

- None (this is the first feature)

## Testing Strategy

### Manual Testing
- Clean clone, `corepack enable`, `pnpm install` — should succeed
- `pnpm run build`, `pnpm run lint`, `pnpm run test` — all pass
