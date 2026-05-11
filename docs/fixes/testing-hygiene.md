---
id: testing-hygiene
title: "Testing hygiene fixes"
status: completed
severity: minor
reported: 2026-04-22T13:10:00.000Z
resolved: 2026-04-22T16:10:00.000Z
resolution:
  rootCause: Accumulated testing issues — flaky timers, missing mock cleanup, inconsistent file placement
  fix:
    - Fake timer leak fixed with try/finally
    - Real timing delay replaced with re-await
    - clearAllMocks added to api-client tests
    - test:unit script for fast dev feedback
    - Misplaced test file moved to __tests__/
    - Real I/O dependency documented
  filesModified:
    - src/sdk/__tests__/credentials.test.ts
    - src/sdk/__tests__/auth-server.test.ts
    - src/sdk/__tests__/api-client.test.ts
    - src/core/__tests__/templates.test.ts
    - src/commands/__tests__/task-update.test.ts
    - package.json
archived: true
---

# Fix: Testing hygiene fixes

## Tasks

### 1. Fix fake timer leak in credentials.test.ts
status: completed
commitSha: 3acf00b

### 2. Remove real timing delay in auth-server.test.ts
status: completed
commitSha: 3acf00b

### 3. Add vi.clearAllMocks() in api-client.test.ts
status: completed
commitSha: 3acf00b

### 4. Separate integration tests from unit tests
status: completed
commitSha: 3acf00b

Added test:unit script to package.json.

### 5. Move task-update.test.ts to __tests__/ directory
status: completed
commitSha: 3acf00b

### 6. Document real I/O dependency in templates.test.ts
status: completed
commitSha: 3acf00b

### 7. Add test category separation to vitest config
status: completed
commitSha: 3acf00b

Combined with task 4 — test:unit script excludes auth-server and e2e tests.
