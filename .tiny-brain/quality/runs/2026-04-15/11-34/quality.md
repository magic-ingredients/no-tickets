---
run_date: 2026-04-15
issue_count: 25
---

# Quality Run - 2026-04-15

## Summary

**Issues Found:** 25
**Analyzer Issues:** 0
**Agent Issues:** 25

## Agent Breakdown

| Agent | Issues |
|-------|--------|
| code-quality | 12 |
| performance | 4 |
| security | 5 |
| testing | 4 |

```json
{
  "agentFindings": {
    "code-quality": {
      "issueCount": 12
    },
    "performance": {
      "issueCount": 4
    },
    "security": {
      "issueCount": 5
    },
    "testing": {
      "issueCount": 4
    }
  },
  "analyzersRun": [],
  "issues": [
    {
      "category": "Maintainability",
      "severity": "major",
      "file": "src/client.ts",
      "line": 131,
      "message": "Repeated type assertion data as Record<string, unknown> across four private parse functions violates no-type-assertions guideline",
      "suggestion": "Extract a shared helper that validates and narrows the response data type",
      "effort": "small",
      "theme": "type-safety",
      "evidence": "data as Record<string, unknown>",
      "scoreImpact": 4,
      "effortHours": 1,
      "ruleId": "AGENT-002",
      "source": "llm"
    },
    {
      "category": "Reliability",
      "severity": "major",
      "file": "src/core/diff.ts",
      "line": 103,
      "message": "Meta field equality uses JSON.stringify which is key-order sensitive - semantically identical objects with different insertion order reported as changed",
      "suggestion": "Use a deterministic deep-equality helper instead of JSON.stringify",
      "effort": "small",
      "theme": "correctness",
      "evidence": "JSON.stringify(a.meta) \\!== JSON.stringify(b.meta)",
      "scoreImpact": 5,
      "effortHours": 1,
      "ruleId": "AGENT-003",
      "source": "llm"
    },
    {
      "category": "Maintainability",
      "severity": "major",
      "file": "src/core/state.ts",
      "line": 44,
      "message": "Orphan-feature detection computes orphanFeatures but the if-body is empty - dead code adding cognitive overhead",
      "suggestion": "Remove the dead code block or wire up an actual warning",
      "effort": "trivial",
      "theme": "dead-code",
      "evidence": "const orphanFeatures = parsed.features.filter(...)",
      "scoreImpact": 3,
      "effortHours": 0.5,
      "ruleId": "AGENT-004",
      "source": "llm"
    },
    {
      "category": "Documentation",
      "severity": "major",
      "file": "src/cli.ts",
      "line": 18,
      "message": "Orphaned JSDoc block intended for parseArgs sits above runCli function instead",
      "suggestion": "Move JSDoc to correct function or remove if not needed",
      "effort": "trivial",
      "theme": "documentation",
      "evidence": "/** Parses CLI arguments... */ export async function runCli()",
      "scoreImpact": 2,
      "effortHours": 0.25,
      "ruleId": "AGENT-001",
      "source": "llm"
    },
    {
      "category": "Maintainability",
      "severity": "minor",
      "file": "src/cli.ts",
      "line": 32,
      "message": "Version string 2.0.0 is hardcoded and will silently diverge from package.json after release bumps",
      "suggestion": "Read version from package.json at build or runtime",
      "effort": "small",
      "theme": "maintainability",
      "evidence": "version: \"2.0.0\"",
      "scoreImpact": 2,
      "effortHours": 0.5,
      "ruleId": "AGENT-005",
      "source": "llm"
    },
    {
      "category": "Maintainability",
      "severity": "minor",
      "file": "src/commands/task-update.ts",
      "line": 59,
      "message": "phaseRaw narrowed via includes check then immediately re-asserted with as TaskPhase - redundant type assertion",
      "suggestion": "Use a proper type guard function instead of type assertion",
      "effort": "trivial",
      "theme": "type-safety",
      "evidence": "as TaskPhase | undefined",
      "scoreImpact": 2,
      "effortHours": 0.5,
      "ruleId": "AGENT-006",
      "source": "llm"
    },
    {
      "category": "Maintainability",
      "severity": "minor",
      "file": "src/commands/task-update.ts",
      "line": 50,
      "message": "statusRaw uses same double-assertion pattern - isTaskStatus type guard exists in parser.ts but not shared",
      "suggestion": "Reuse the existing isTaskStatus type guard from parser.ts",
      "effort": "trivial",
      "theme": "type-safety",
      "evidence": "as TaskStatus | undefined",
      "scoreImpact": 2,
      "effortHours": 0.25,
      "ruleId": "AGENT-007",
      "source": "llm"
    },
    {
      "category": "Documentation",
      "severity": "minor",
      "file": "src/mcp/create-server.ts",
      "line": 1,
      "message": "Stub MCP server module has no documentation explaining planned tool interface or tracking ticket",
      "suggestion": "Add module header with feature ticket reference and planned interface",
      "effort": "trivial",
      "theme": "documentation",
      "evidence": "// stub module",
      "scoreImpact": 1,
      "effortHours": 0.25,
      "ruleId": "AGENT-008",
      "source": "llm"
    },
    {
      "category": "Documentation",
      "severity": "minor",
      "file": "src/core/schemas.ts",
      "line": 1,
      "message": "No module header explaining relationship between Zod schemas and parallel TypeScript types in types.ts",
      "suggestion": "Add brief module doc explaining schema/type relationship",
      "effort": "trivial",
      "theme": "documentation",
      "evidence": "import { z } from \"zod\"",
      "scoreImpact": 1,
      "effortHours": 0.25,
      "ruleId": "AGENT-009",
      "source": "llm"
    },
    {
      "category": "Maintainability",
      "severity": "info",
      "file": "src/core/validator.ts",
      "line": 175,
      "message": "formatSuggestion uses chain of if statements on field name strings instead of typed lookup map",
      "suggestion": "Replace if-chain with a typed Record lookup",
      "effort": "small",
      "theme": "maintainability",
      "evidence": "if (field === \"title\") ...",
      "scoreImpact": 1,
      "effortHours": 1,
      "ruleId": "AGENT-010",
      "source": "llm"
    },
    {
      "category": "Maintainability",
      "severity": "info",
      "file": "src/core/state.ts",
      "line": 35,
      "message": "epicIds Set built solely for dead orphan filter - can be removed with the dead code",
      "suggestion": "Remove alongside the dead orphan-feature block",
      "effort": "trivial",
      "theme": "dead-code",
      "evidence": "const epicIds = new Set(...)",
      "scoreImpact": 1,
      "effortHours": 0.25,
      "ruleId": "AGENT-011",
      "source": "llm"
    },
    {
      "category": "Testing",
      "severity": "info",
      "file": "src/core/__tests__/diff.test.ts",
      "line": 1,
      "message": "No test for duplicate feature IDs across multiple epics - indexFeatures silently overwrites on ID collision",
      "suggestion": "Add test case for duplicate feature ID across epics",
      "effort": "small",
      "theme": "test-coverage",
      "evidence": "indexFeatures",
      "scoreImpact": 1,
      "effortHours": 1,
      "ruleId": "AGENT-012",
      "source": "llm"
    },
    {
      "category": "Performance",
      "severity": "minor",
      "file": "src/core/diff.ts",
      "line": 103,
      "message": "JSON.stringify used to compare meta objects on every feature diff, serializing potentially large nested objects for every feature comparison",
      "suggestion": "Replace JSON.stringify comparison with a shallow key-value equality check or use a lightweight structural comparison that avoids full serialization",
      "effort": "small",
      "theme": "algorithm",
      "evidence": "if (JSON.stringify(prev.meta) \\!== JSON.stringify(curr.meta)) {\n  changes['meta'] = { from: prev.meta, to: curr.meta };\n}",
      "scoreImpact": 3,
      "effortHours": 1,
      "ruleId": "PERF-001",
      "source": "llm"
    },
    {
      "category": "Performance",
      "severity": "minor",
      "file": "src/core/parser.ts",
      "line": 100,
      "message": "extractSection compiles a new RegExp on every invocation. It is called 3-4 times per file (Goals, Tasks, Acceptance Criteria, Description), so regexp construction is repeated for each section of every parsed document",
      "suggestion": "Memoize the compiled regexp by section name using a Map<string, RegExp> cache at module scope, or restructure to compile the regex once and reuse it",
      "effort": "trivial",
      "theme": "algorithm",
      "evidence": "export function extractSection(body: string, sectionName: string): string | undefined {\n  const escaped = escapeRegex(sectionName);\n  const regex = new RegExp(`^##\\\\s+${escaped}\\\\s*$`, 'm');\n  const match = regex.exec(body);",
      "scoreImpact": 2,
      "effortHours": 0.5,
      "ruleId": "PERF-002",
      "source": "llm"
    },
    {
      "category": "Performance",
      "severity": "minor",
      "file": "src/core/state.ts",
      "line": 44,
      "message": "A second full linear scan of parsed.features is performed solely to detect orphan features that are then silently dropped with no action taken. This wasted O(n) allocation and iteration is entirely dead code.",
      "suggestion": "Remove the orphanFeatures filter entirely since the result is never used. If orphan detection is needed in future, integrate it into the existing loop above rather than a separate pass",
      "effort": "trivial",
      "theme": "algorithm",
      "evidence": "const orphanFeatures = parsed.features.filter((f) => \\!epicIds.has(f.frontmatter.epic));\nif (orphanFeatures.length > 0) {\n  // Orphan features are silently dropped — validation should catch this upstream\n}",
      "scoreImpact": 2,
      "effortHours": 0.25,
      "ruleId": "PERF-003",
      "source": "llm"
    },
    {
      "category": "Performance",
      "severity": "major",
      "file": "src/core/validator.ts",
      "line": 120,
      "message": "Tasks are sorted with Array.sort() inside validateTasks which is called once per feature. For features with many tasks this allocates a new sorted array on every validation run. More critically, tasks are already iterated once in the loop above to detect duplicates, so a second pass is redundant.",
      "suggestion": "Track the expected sequence incrementally inside the existing loop using a counter, eliminating the need for the sort and the second pass entirely. This reduces the gap-check from O(n log n) to O(n) and removes the extra allocation.",
      "effort": "small",
      "theme": "algorithm",
      "evidence": "if (feature.tasks.length > 0) {\n  const numbers = feature.tasks.map((t) => t.number).sort((a, b) => a - b);\n  for (let i = 0; i < numbers.length; i++) {\n    const expected = i + 1;\n    if (numbers[i] \\!== expected) {",
      "scoreImpact": 5,
      "effortHours": 1,
      "ruleId": "PERF-004",
      "source": "llm"
    },
    {
      "category": "Security",
      "severity": "major",
      "file": "src/client.ts",
      "line": 66,
      "message": "URL path injection via unsanitized teamId. The teamId value from SyncConfig is interpolated directly into the URL path without validation or encoding, allowing path traversal or request routing manipulation if a malicious teamId is supplied (e.g., containing \"../\" or query delimiters).",
      "suggestion": "Apply encodeURIComponent() to teamId (and all path parameters) before interpolating into the URL. Validate that teamId matches an expected pattern (e.g., kebab-case alphanumeric) at config load time.",
      "effort": "trivial",
      "theme": "input-validation",
      "evidence": "const response = await fetch(`${this.config.apiUrl}/api/v1/teams/${teamId}`, {",
      "references": [
        "CWE-22",
        "CWE-918"
      ],
      "scoreImpact": 8,
      "effortHours": 0.5,
      "ruleId": "SEC-001",
      "source": "llm"
    },
    {
      "category": "Security",
      "severity": "major",
      "file": "src/client.ts",
      "line": 100,
      "message": "URL path injection via unsanitized taskId. The taskId parameter is interpolated directly into the URL path in the taskUpdate method without encoding. An attacker-controlled taskId could manipulate the request target.",
      "suggestion": "Apply encodeURIComponent(taskId) before interpolating into the URL path. Add input validation to reject taskId values that do not match expected formats.",
      "effort": "trivial",
      "theme": "input-validation",
      "evidence": "const response = await fetch(`${this.config.apiUrl}/api/v1/tasks/${taskId}`, {",
      "references": [
        "CWE-22",
        "CWE-918"
      ],
      "scoreImpact": 8,
      "effortHours": 0.5,
      "ruleId": "SEC-002",
      "source": "llm"
    },
    {
      "category": "Security",
      "severity": "minor",
      "file": "src/client.ts",
      "line": 44,
      "message": "Server-Side Request Forgery risk via configurable apiUrl. The apiUrl from SyncConfig is used as the base for all HTTP requests without validation. If an attacker can control the config (e.g., via a malicious config.json), they can redirect API calls to arbitrary hosts, potentially reaching internal services.",
      "suggestion": "Validate apiUrl against an allowlist of known API domains, or at minimum validate it uses HTTPS and is a well-formed URL. Consider rejecting private/internal IP ranges.",
      "effort": "small",
      "theme": "input-validation",
      "evidence": "const response = await fetch(`${this.config.apiUrl}/api/v1/snapshots`, {",
      "references": [
        "CWE-918"
      ],
      "scoreImpact": 5,
      "effortHours": 2,
      "ruleId": "SEC-003",
      "source": "llm"
    },
    {
      "category": "Security",
      "severity": "minor",
      "file": "src/client.ts",
      "line": 112,
      "message": "Error message from caught exceptions is passed through to the caller without sanitization. While currently the error is returned in a structured result object, internal error details (stack traces, file paths, network errors) could leak sensitive information about the server environment to upstream consumers.",
      "suggestion": "Sanitize or genericize error messages before returning them. Log the full error internally but return a generic message like \"Task update failed\" to the caller.",
      "effort": "trivial",
      "theme": "data-exposure",
      "evidence": "return { success: false, error: err instanceof Error ? err.message : \"Unknown error\" };",
      "references": [
        "CWE-200"
      ],
      "scoreImpact": 3,
      "effortHours": 0.5,
      "ruleId": "SEC-004",
      "source": "llm"
    },
    {
      "category": "Security",
      "severity": "info",
      "file": "src/client.ts",
      "line": 117,
      "message": "Bearer token is included in every request header but there is no mechanism to validate token expiry, rotation, or revocation client-side. The token persists in the SyncConfig object for the lifetime of the client instance with no refresh logic.",
      "suggestion": "Consider implementing token expiry checks before requests and a refresh mechanism. At minimum, document the expected token lifecycle so consumers handle rotation appropriately.",
      "effort": "medium",
      "theme": "auth-hardening",
      "evidence": "return { \"Authorization\": `Bearer ${this.config.token}` };",
      "references": [
        "CWE-613"
      ],
      "scoreImpact": 2,
      "effortHours": 4,
      "ruleId": "SEC-005",
      "source": "llm"
    },
    {
      "category": "Testing",
      "severity": "minor",
      "file": "src/core/__tests__/templates.test.ts",
      "line": 10,
      "message": "Real file I/O in tests via readFileSync reads template files from disk, making these tests flaky if the working directory or file layout changes",
      "suggestion": "Mock the fs module with vi.mock(\"fs\") and return fixture strings, or use an in-memory filesystem (memfs). Alternatively accept these as integration-level tests and run them under a separate test workspace/tag.",
      "effort": "small",
      "theme": "flaky-tests",
      "evidence": "function readTemplate(name: string): string {\n  return readFileSync(join(TEMPLATES_DIR, name), \"utf-8\");\n}",
      "scoreImpact": 2.5,
      "effortHours": 1.5,
      "ruleId": "TEST-flaky-real-io",
      "source": "llm"
    },
    {
      "category": "Testing",
      "severity": "major",
      "file": "src/__tests__",
      "line": 1,
      "message": "No test category separation: all tests run under a single vitest command with no workspace split, tags, or separate scripts. Fast pure-function tests (parser, state, diff) and slower I/O-backed tests (templates) are indistinguishable to the runner, preventing granular feedback loops (edit vs commit vs regression).",
      "suggestion": "Add vitest workspaces or use vitest --project flags to separate unit tests (no I/O) from integration-level tests. Add npm scripts: \"test:unit\" for pure function tests and keep \"test\" for the full suite. Use vitest test tags or file naming conventions (*.integration.test.ts) so engineers can run only fast tests on save.",
      "effort": "small",
      "theme": "test-organisation",
      "evidence": "\"scripts\": {\n  \"test\": \"vitest run\"\n}",
      "scoreImpact": 5,
      "effortHours": 2,
      "ruleId": "TEST-organisation",
      "source": "llm"
    },
    {
      "category": "Testing",
      "severity": "minor",
      "file": "src/__tests__/client.test.ts",
      "line": 38,
      "message": "Type assertions on mock call arguments (as string, as RequestInit, as Record<string, string>) bypass TypeScript safety in tests. If the implementation changes the argument type, the tests will silently pass with wrong types.",
      "suggestion": "Use typed mock helpers or introduce a helper function that extracts and validates the call arguments with proper type guards. Consider using vi.mocked() or creating typed wrappers rather than casting with as.",
      "effort": "small",
      "theme": "brittle-tests",
      "evidence": "const url = fetchSpy.mock.calls[0]?.[0] as string;\nconst options = fetchSpy.mock.calls[0]?.[1] as RequestInit;\nexpect((options.headers as Record<string, string>)[\"Authorization\"]).toBe(\"Bearer nt_test_token\");",
      "scoreImpact": 2,
      "effortHours": 1,
      "ruleId": "TEST-brittle-type-assertions",
      "source": "llm"
    },
    {
      "category": "Testing",
      "severity": "info",
      "file": "src/core/__tests__/parser-mutants.test.ts",
      "line": 1,
      "message": "Mutant-killer tests are co-located alongside primary parser tests in a separate file without clear documentation of the test pyramid layer they occupy. While mutation testing is valuable and the coverage is excellent, the split into a separate mutants file creates duplication of describe block names and makes it harder to see at a glance what behaviour is actually being verified vs what is purely a mutant-killing regression.",
      "suggestion": "Consider consolidating mutant-killer tests into the primary parser.test.ts and validator.test.ts files, using a comment block to mark them. Or document in a README that *-mutants.test.ts files are Stryker regression tests. Either way, ensure the separate file is a deliberate convention rather than accumulating technical debt.",
      "effort": "trivial",
      "theme": "test-organisation",
      "evidence": "// src/core/__tests__/parser-mutants.test.ts (280+ lines)\n// src/core/__tests__/validator-mutants.test.ts (190+ lines)\n// Duplicates describe block names from parser.test.ts and validator.test.ts",
      "scoreImpact": 1,
      "effortHours": 0.5,
      "ruleId": "TEST-organisation-mutant-files",
      "source": "llm"
    }
  ]
}
```
