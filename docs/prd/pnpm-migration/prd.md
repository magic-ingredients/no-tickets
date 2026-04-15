---
id: pnpm-migration
title: "Migrate to pnpm and add GitHub Actions CI"
version: 1.0.0
status: not_started
created: 2026-04-15
updated: 2026-04-15
author: Claude Code
---

# Migrate to pnpm and add GitHub Actions CI

## Purpose and Goals

Switch the repository's package manager from npm to pnpm and establish CI via GitHub Actions. pnpm offers faster installs, stricter dependency resolution, and disk-efficient storage. Adding CI ensures every push and PR is validated automatically.

- Replace npm with pnpm as the sole package manager
- Enforce pnpm version via corepack
- Add GitHub Actions workflows for CI (typecheck, lint, test)

## User Needs

### Target Audience
- Contributors to the no-tickets repository
- CI/CD systems building and validating the project

### User Stories

1. As a contributor, I want `pnpm install` to work out of the box so that I don't need to guess which package manager to use
2. As a maintainer, I want CI to run typecheck, lint, and tests on every PR so that regressions are caught before merge
3. As a contributor, I want CI to use pnpm caching so that pipeline runs are fast

## Features and Functionality

### Feature 1: pnpm Migration
**File**: [features/pnpm-swap.md](features/pnpm-swap.md)
**Status**: planned
**Description**: Replace npm with pnpm — swap lockfile, add corepack packageManager field, update README and any npm references

### Feature 2: GitHub Actions CI
**File**: [features/github-actions-ci.md](features/github-actions-ci.md)
**Status**: planned
**Description**: Add GitHub Actions workflow that runs typecheck, lint, and tests on push/PR

## Release Criteria

### Functional Requirements
- [ ] `pnpm install` succeeds from a clean clone
- [ ] `pnpm run build`, `pnpm run lint`, `pnpm run test` all pass
- [ ] CI workflow passes on push to main and on PRs
- [ ] package-lock.json is removed

### Technical Requirements
- [ ] `packageManager` field set in package.json for corepack
- [ ] pnpm-lock.yaml committed
- [ ] GitHub Actions workflow uses pnpm caching

## Constraints and Dependencies

### Dependencies
- Node.js >= 18 (corepack ships with Node 16.13+)
- GitHub Actions runner access

### Known Limitations
- Existing contributors will need to run `corepack enable` once
