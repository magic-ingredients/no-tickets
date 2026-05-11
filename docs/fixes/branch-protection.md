---
id: branch-protection
title: "Add branch protection rules to main before going public"
status: not_started
severity: major
reported: 2026-04-15T09:00:00.000Z
resolved: null
---

# Fix: Add branch protection rules to main

GitHub rulesets require either a paid plan or a public repo. Once the repo is made public, configure branch protection for main.

## Tasks

### 1. Make repo public and create branch protection ruleset
status: not_started

After making the repo public, create a GitHub ruleset for `main` with:
- Require 1 approving PR review before merge
- Dismiss stale reviews on push
- Require review thread resolution
- Require `validate` status check to pass (CI workflow)
- Prevent force pushes (non-fast-forward)

Use `gh api repos/magic-ingredients/no-tickets/rulesets` to create the ruleset.
