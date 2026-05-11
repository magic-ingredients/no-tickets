---
id: qip-security
title: "Security quality issues"
status: superseded
severity: major
reported: 2026-04-15T11:41:19.434Z
resolved: 2026-04-22T13:10:00.000Z
archived: true
resolution:
  rootCause: All 4 issues are in src/client.ts which is being deleted as legacy code
  fix:
    - Superseded by fix delete-legacy-client
  filesModified: []
---

# Fix: Security quality issues

Source run: 2026-04-15T11-34

**All tasks superseded** — `src/client.ts` is being deleted entirely (see fix `delete-legacy-client`).

## Tasks

### 1. [major] src/client.ts:66
status: superseded
commitSha: null

URL path injection via unsanitized teamId.

### 2. [major] src/client.ts:100
status: superseded
commitSha: null

URL path injection via unsanitized taskId.

### 3. [minor] src/client.ts:44
status: superseded
commitSha: null

Server-Side Request Forgery risk via configurable apiUrl.

### 4. [minor] src/client.ts:112
status: superseded
commitSha: null

Error message leaking without sanitization.
