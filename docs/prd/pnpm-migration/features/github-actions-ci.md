---
id: github-actions-ci
prd_id: pnpm-migration
number: 2
title: Add GitHub Actions CI workflow
status: completed
created: 2026-04-15
updated: 2026-04-15
---

# Feature: Add GitHub Actions CI workflow

## Description

Add a GitHub Actions workflow that validates the project on every push to main and on pull requests. The workflow should run typecheck, lint, and tests using pnpm with dependency caching for fast runs.

## Acceptance Criteria

- [ ] `.github/workflows/ci.yml` exists
- [ ] Workflow triggers on push to main and on PRs
- [ ] Workflow runs: pnpm install, typecheck, lint, test
- [ ] pnpm store is cached between runs
- [ ] Workflow uses corepack to get the correct pnpm version

## Tasks

### 1. Add CI workflow file
status: completed
commitSha: e98b0c9

Create `.github/workflows/ci.yml` with a job that installs dependencies, runs typecheck, lint, and tests.

**Files to modify/create:**
- `.github/workflows/ci.yml`

**Expected changes:**
- Node.js setup with corepack enable
- pnpm store caching via `actions/setup-node` built-in caching
- Sequential steps: install, typecheck (`pnpm run build`), lint (`pnpm run lint`), test (`pnpm run test`)

## Dependencies

- **Feature: pnpm-swap** — CI workflow depends on pnpm being the package manager

## Testing Strategy

### Manual Testing
- Push branch and verify workflow runs green on GitHub
- Open a PR and verify checks appear
