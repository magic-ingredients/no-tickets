---
id: oss-launch
prd_id: no-tickets-client
number: 7
title: Open Source Launch & npm Publish
status: completed
created: 2026-04-17
updated: 2026-04-22
---

# Feature: Open Source Launch & npm Publish

## Description

Remaining tasks from the service repo's Feature 11 (Repo Restructure & Open Source Launch) that are scoped to the public `no-tickets` repo and the `tiny-brain` repo. Covers CI setup, npm publishing, and wiring tiny-brain as a consumer.

Tasks 1-4 of the original Feature 11 (create repo, extract code, add MCP entry point, wire service repo) are already completed. These are the remaining tasks to close out the OSS launch.

## Acceptance Criteria

- [ ] GitHub Actions CI: build + test + lint + typecheck on PR
- [ ] GitHub Actions: npm publish on tag/release with provenance
- [ ] `@magic-ingredients/no-tickets` v2.0.0 published to npm registry
- [ ] `npx no-tickets init` works from the published package
- [ ] tiny-brain repo consumes `@magic-ingredients/no-tickets` as dependency
- [ ] tiny-brain repo has LICENSE (Apache 2.0), README.md, CONTRIBUTING.md
- [ ] No hardcoded secrets or internal URLs in tiny-brain repo

## Tasks

### 1. Set up CI for public repo
status: completed
commitSha: 2dba7d2

ci.yml: typecheck + build + lint + test on PR, reusable via workflow_call.
publish.yml: triggered on release, validates tag matches package.json version,
runs CI, publishes with --provenance --access public. tsconfig.build.json
excludes tests from dist. files field in package.json scopes the tarball.

### 2. Publish initial npm release
status: completed
commitSha: f049b46

Published @magic-ingredients/no-tickets@2.0.0 to npm via GitHub release workflow.

### 3. Wire tiny-brain to consume npm package
status: superseded
commitSha: null

Tracked in tiny-brain repo, not here.

### 4. Update tiny-brain for open source
status: superseded
commitSha: null

Tracked in tiny-brain repo, not here.

## Dependencies

- Features 1-5 of this PRD (auth, schemas, push, and MCP must work before publishing)

## Testing Strategy

### Validation
- npm package installs and runs (`npx no-tickets --help`)
- tiny-brain's import of SDK types compiles
- CI passes on both repos
- Published package matches local build output
