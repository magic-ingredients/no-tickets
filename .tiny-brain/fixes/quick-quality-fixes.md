---
id: quick-quality-fixes
title: "Batch trivial/small quality fixes"
status: completed
severity: minor
reported: 2026-04-22T13:10:00.000Z
resolved: 2026-04-22T16:04:00.000Z
resolution:
  rootCause: Accumulated minor quality issues from quality analysis run
  fix:
    - Batch of 11 fixes in one commit plus deepEqual correction
  filesModified:
    - src/sdk/api-client.ts
    - src/cli.ts
    - src/core/parser.ts
    - src/mcp/tools/push.ts
    - src/agent-detect.ts
    - src/core/diff.ts
    - src/core/state.ts
    - src/sdk/credentials.ts
archived: true
---

# Fix: Batch trivial/small quality fixes

## Tasks

### 1. Remove unsafe headers cast in api-client.ts
status: completed
commitSha: 2eb7272

### 2. Load version dynamically in CLI
status: completed
commitSha: 2eb7272

### 3. Guard parseMeta against non-plain objects
status: completed
commitSha: 2eb7272

### 4. Wrap JSON.parse in MCP push handler
status: completed
commitSha: 2eb7272

### 5. Remove redundant `as AssigneeType` casts
status: completed
commitSha: 2eb7272

### 6. Replace JSON.stringify meta equality with deep-equal
status: completed
commitSha: 1c09314

### 7. Replace filter().length with reduce for task count
status: completed
commitSha: 2eb7272

### 8. Remove unnecessary Buffer.from in readStdin
status: completed
commitSha: 2eb7272

### 9. Add path validation to MCP validate tool
status: superseded
commitSha: null

Already handled by fix extract-read-notickets-dir (shared readNoTicketsDir with path validation).

### 10. Set credentials directory permissions to 0o700
status: completed
commitSha: 2eb7272

### 11. Sanitize CLI error output
status: completed
commitSha: 2eb7272
