---
id: qip-code-quality
title: "Code-quality quality issues"
status: superseded
severity: critical
reported: 2026-04-15T08:44:32.354Z
resolved: 2026-04-22T13:10:00.000Z
archived: true
resolution:
  rootCause: Legacy quality run — most issues fixed during Feature 3-6 implementation
  fix:
    - Tasks 1, 4, 6 fixed during feature work
    - Tasks 2, 3, 5 superseded by new fix docs (delete-legacy-client, quick-quality-fixes)
  filesModified: []
---

# Fix: Code-quality quality issues

Source run: 2026-04-15T08-39

## Tasks

### 1. [critical] bin/no-tickets.js:46
status: superseded
commitSha: null

runCli is imported from dist/cli.js but is never exported from src/cli.ts — this will throw a runtime error whenever the CLI is invoked with arguments

**Fixed:** runCli is now exported and fully functional.

### 2. [major] src/client.ts:133
status: superseded
commitSha: null

Multiple type assertions (as Record<string, unknown>) are used to access unknown API response data instead of leveraging the Zod dependency already present in the project

**Superseded by:** fix delete-legacy-client (client.ts being deleted entirely)

### 3. [major] src/commands/task-update.ts:50
status: superseded
commitSha: null

statusRaw and phaseRaw are validated manually then cast with as TaskStatus and as TaskPhase

**Superseded by:** tracked in quick-quality-fixes if still needed after client.ts deletion

### 4. [major] src/core/state.ts:44
status: superseded
commitSha: null

orphanFeatures is computed via a filter but immediately discarded

**Fixed:** Dead code removed in commit 69d6b40.

### 5. [minor] src/core/diff.ts:103
status: superseded
commitSha: null

Deep equality of the meta field is determined by JSON.stringify comparison

**Superseded by:** fix quick-quality-fixes task 6

### 6. [minor] src/mcp/create-server.ts:1
status: superseded
commitSha: null

The MCP server module is an empty stub

**Fixed:** MCP server rewritten with 3 real tools in commit 2f3b09b.
