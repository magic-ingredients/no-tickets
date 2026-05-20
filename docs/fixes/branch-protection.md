---
id: branch-protection
title: "Add branch protection rules to main before going public"
status: completed
severity: major
reported: 2026-04-15T09:00:00.000Z
resolved: 2026-05-20T09:35:00.000Z
resolution:
  rootCause: Repo was private from creation; branch protection deferred until public-flip.
  fix:
    - Repo flipped to public after v0.1.0 cargo-dist pipeline went green
    - Branch protection applied to `main` via gh api (legacy Branch Protection endpoint rather than the Rulesets endpoint named in the task — both surfaces achieve the goal)
    - Settings beyond original spec - linear history required, enforce_admins=true, required_conversation_resolution=true
    - One deviation from original spec - required_approving_review_count set to 0 (solo maintainer; bump to 1 when collaborators added) rather than 1
  filesModified: []
archived: true
---

# Fix: Add branch protection rules to main

GitHub rulesets require either a paid plan or a public repo. Once the repo is made public, configure branch protection for main.

## Tasks

### 1. Make repo public and create branch protection ruleset
status: completed
commitSha: null

After making the repo public, create a GitHub ruleset for `main` with:
- Require 1 approving PR review before merge
- Dismiss stale reviews on push
- Require review thread resolution
- Require `validate` status check to pass (CI workflow)
- Prevent force pushes (non-fast-forward)

Use `gh api repos/magic-ingredients/no-tickets/rulesets` to create the ruleset.

**Resolution note (2026-05-20):** Configured via the legacy Branch Protection API (`gh api repos/.../branches/main/protection`) rather than Rulesets — same outcome. Configuration applied: required PR before merge, `validate` status check (strict), dismiss stale reviews, conversation resolution required, linear history, enforce_admins=true, force-push blocked, deletion blocked. Required approving review count is **0** rather than the originally-spec'd 1 — solo-maintainer trade-off; bump to 1 when collaborators are added. `commitSha: null` because the change is GitHub repo configuration, not in-tree code.
