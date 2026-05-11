---
id: delete-legacy-client
title: "Delete legacy NoTicketsClient (client.ts)"
status: completed
severity: major
reported: 2026-04-22T13:10:00.000Z
resolved: null
archived: true
---

# Fix: Delete legacy NoTicketsClient (client.ts)

`src/client.ts` is a legacy HTTP client superseded by `src/sdk/api-client.ts`. It duplicates HTTP plumbing (auth headers, JSON headers, fetch patterns, error parsing) and carries 4 unresolved security issues (path injection, SSRF). The new push flow uses `api-client.ts` exclusively.

Deleting it eliminates 5 quality issues (CQ-001, CQ-003 partial, CQ-010) and all 4 issues from the old `qip-security` fix.

## Tasks

### 1. Delete src/client.ts and its test
status: completed
commitSha: 148dd8d

### 2. Remove client.ts references from exports
status: completed
commitSha: 148dd8d

No exports referenced client.ts — verified via grep before deletion.

### 3. Update qip-security fix as superseded
status: completed
commitSha: null

All 4 qip-security tasks marked superseded in the fix doc.
