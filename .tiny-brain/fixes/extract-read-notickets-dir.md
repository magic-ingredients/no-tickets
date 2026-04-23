---
id: extract-read-notickets-dir
title: "Extract and harden shared readNoTicketsDir"
status: completed
severity: major
reported: 2026-04-22T13:10:00.000Z
resolved: 2026-04-22T15:06:00.000Z
resolution:
  rootCause: readNoTicketsDir duplicated in cli.ts and mcp/tools/validate.ts with sequential I/O and no path validation
  fix:
    - Extracted to src/core/fs.ts as shared module
    - Parallelized I/O with Promise.all for stat and readFile
    - Added path validation with cwd + sep check to prevent traversal
  filesModified:
    - src/core/fs.ts
    - src/cli.ts
    - src/mcp/tools/validate.ts
archived: true
---

# Fix: Extract and harden shared readNoTicketsDir

Quality issues addressed: CQ-002, PERF-001, PERF-002, SEC-001, SEC-007.

## Tasks

### 1. Extract readNoTicketsDir to shared module
status: completed
commitSha: f48de5e

### 2. Parallelize I/O with Promise.all
status: completed
commitSha: f48de5e

### 3. Add path validation
status: completed
commitSha: f48de5e
