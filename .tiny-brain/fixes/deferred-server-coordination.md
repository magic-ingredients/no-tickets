---
id: deferred-server-coordination
title: "Deferred fixes requiring server-side coordination"
status: completed
severity: major
reported: 2026-04-22T13:10:00.000Z
resolved: 2026-04-22T18:36:00.000Z
resolution:
  rootCause: Path injection risk and error message leaking in API client
  fix:
    - Tasks 1-2 handled server-side (superseded)
    - encodeURIComponent applied to all path params
    - Error messages truncated to 200 chars
  filesModified:
    - src/sdk/api-client.ts
    - src/commands/token.ts
archived: true
---

# Fix: Deferred fixes requiring server-side coordination

## Tasks

### 1. Add CSRF state parameter to OAuth callback
status: superseded
commitSha: null

### 2. Resolve placeholder email in init-auth
status: superseded
commitSha: null

### 3. Encode path parameters in API client
status: completed
commitSha: 08c43ac

### 4. Sanitize server error messages
status: completed
commitSha: 08c43ac
