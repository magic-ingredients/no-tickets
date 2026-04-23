---
run_date: 2026-04-22
issue_count: 32
---

# Quality Run - 2026-04-22

## Summary

**Issues Found:** 32
**Analyzer Issues:** 0
**Agent Issues:** 32

## Agent Breakdown

| Agent | Issues |
|-------|--------|
| code-quality | 12 |
| performance | 5 |
| security | 8 |
| testing | 7 |

```json
{
  "agentFindings": {
    "code-quality": {
      "issueCount": 12
    },
    "performance": {
      "issueCount": 5
    },
    "security": {
      "issueCount": 8
    },
    "testing": {
      "issueCount": 7
    }
  },
  "analyzersRun": [],
  "issues": [
    {
      "category": "Maintainability",
      "severity": "major",
      "file": "src/client.ts",
      "line": 35,
      "message": "NoTicketsClient class duplicates HTTP plumbing already present in src/sdk/api-client.ts: authHeaders, jsonHeaders, fetch call patterns, and JSON parsing helpers are re-implemented with nearly identical logic",
      "suggestion": "Consolidate HTTP communication into a single shared module (api-client or a new http-client helper). NoTicketsClient should delegate to createApiClient or be removed if it is a legacy entry point",
      "effort": "medium",
      "theme": "duplication",
      "evidence": "private authHeaders(): Record<string, string> {\n  return { Authorization: `Bearer ${this.config.token}` };\n}\nprivate jsonHeaders(): Record<string, string> {\n  return { Authorization: `Bearer ${this.config.token}`, 'Content-Type': 'application/json' };\n}",
      "scoreImpact": 5,
      "effortHours": 4,
      "ruleId": "CQ-001",
      "source": "llm"
    },
    {
      "category": "Maintainability",
      "severity": "major",
      "file": "src/cli.ts",
      "line": 27,
      "message": "readNoTicketsDir is duplicated verbatim between src/cli.ts (line 27) and src/mcp/tools/validate.ts (line 7). The two implementations are identical: same directory traversal, same stat/readdir calls, same error handling",
      "suggestion": "Extract readNoTicketsDir into a shared utility module (e.g., src/io/read-dir.ts) and import it in both callers",
      "effort": "small",
      "theme": "duplication",
      "evidence": "async function readNoTicketsDir(dir: string): Promise<readonly FileEntry[]> {\n  const entries: FileEntry[] = [];\n  let items: string[];\n  try { items = await readdir(dir); } catch { return []; }\n  for (const item of items) { ... }\n}",
      "scoreImpact": 4,
      "effortHours": 1,
      "ruleId": "CQ-002",
      "source": "llm"
    },
    {
      "category": "Maintainability",
      "severity": "major",
      "file": "src/commands/token.ts",
      "line": 44,
      "message": "authHeaders and jsonHeaders helper functions duplicate the same pattern defined in src/client.ts (lines 117-128). Three separate files independently construct Authorization and Content-Type header objects with identical logic",
      "suggestion": "Create a single shared buildHeaders utility in the SDK layer (e.g., src/sdk/headers.ts) and import it in token.ts, client.ts, and api-client.ts",
      "effort": "small",
      "theme": "duplication",
      "evidence": "function authHeaders(sessionToken: string): Record<string, string> {\n  return { Authorization: `Bearer ${sessionToken}` };\n}\nfunction jsonHeaders(sessionToken: string): Record<string, string> {\n  return { Authorization: `Bearer ${sessionToken}`, 'Content-Type': 'application/json' };\n}",
      "scoreImpact": 3,
      "effortHours": 1.5,
      "ruleId": "CQ-003",
      "source": "llm"
    },
    {
      "category": "Maintainability",
      "severity": "major",
      "file": "src/sdk/api-client.ts",
      "line": 82,
      "message": "Type assertion on options.headers spreads an unknown type with an unsafe cast. The spread '...options?.headers as Record<string, string> | undefined' asserts the caller-supplied headers type without verification, silently dropping values if the actual type differs",
      "suggestion": "Either constrain the RequestInit options parameter to require typed headers, or explicitly validate/coerce the headers field before merging",
      "effort": "trivial",
      "theme": "type-safety",
      "evidence": "headers: {\n  ...headers,\n  ...options?.headers as Record<string, string> | undefined,\n},",
      "scoreImpact": 3,
      "effortHours": 0.5,
      "ruleId": "CQ-004",
      "source": "llm"
    },
    {
      "category": "Maintainability",
      "severity": "minor",
      "file": "src/cli.ts",
      "line": 143,
      "message": "Version string '2.0.0' is hardcoded in the version command handler. The package.json version is already loaded dynamically in src/mcp/create-server.ts but the CLI duplicates the value as a magic literal",
      "suggestion": "Load the version dynamically using createRequire (matching the pattern in mcp/create-server.ts) or read it from package.json at startup",
      "effort": "trivial",
      "theme": "magic-numbers",
      "evidence": "case 'version':\n  console.log('2.0.0');\n  break;",
      "scoreImpact": 1.5,
      "effortHours": 0.5,
      "ruleId": "CQ-005",
      "source": "llm"
    },
    {
      "category": "Maintainability",
      "severity": "minor",
      "file": "src/commands/init-auth.ts",
      "line": 18,
      "message": "PLACEHOLDER_EMAIL constant is a temporary stand-in acknowledged by a comment as never being resolved. It causes stale placeholder data to be stored in credentials for every authenticated user",
      "suggestion": "Either return an empty string and handle the absent email gracefully downstream, or update the auth flow to request the email from the server before saving credentials",
      "effort": "medium",
      "theme": "dead-code",
      "evidence": "// Placeholder until server-side /auth/cli returns email in the callback\nconst PLACEHOLDER_EMAIL = 'authenticated@no-tickets.com';",
      "scoreImpact": 2,
      "effortHours": 3,
      "ruleId": "CQ-006",
      "source": "llm"
    },
    {
      "category": "Maintainability",
      "severity": "minor",
      "file": "src/core/diff.ts",
      "line": 103,
      "message": "JSON.stringify is used to compare meta objects for equality. This approach is key-order-sensitive: two semantically identical objects with different key insertion order will report a false change",
      "suggestion": "Use a deep-equality utility (e.g., a recursive isEqual helper or a library like fast-deep-equal) rather than relying on JSON serialisation order",
      "effort": "small",
      "theme": "complexity",
      "evidence": "if (JSON.stringify(prev.meta) !== JSON.stringify(curr.meta)) {\n  changes['meta'] = { from: prev.meta, to: curr.meta };\n}",
      "scoreImpact": 2,
      "effortHours": 1,
      "ruleId": "CQ-007",
      "source": "llm"
    },
    {
      "category": "Maintainability",
      "severity": "minor",
      "file": "src/core/parser.ts",
      "line": 163,
      "message": "parseMeta uses a type assertion (as Readonly<Record<string, unknown>>) after confirming the value is a non-null object. The assertion discards structural knowledge and could mask non-plain-object values (e.g., arrays, class instances)",
      "suggestion": "Add a runtime check that the value is a plain object (e.g., Object.getPrototypeOf(meta) === Object.prototype) before casting, or use a type guard",
      "effort": "trivial",
      "theme": "type-safety",
      "evidence": "function parseMeta(data: Record<string, unknown>): Readonly<Record<string, unknown>> | undefined {\n  const meta = data['meta'];\n  if (typeof meta !== 'object' || meta === null) return undefined;\n  return meta as Readonly<Record<string, unknown>>;\n}",
      "scoreImpact": 1,
      "effortHours": 0.5,
      "ruleId": "CQ-008",
      "source": "llm"
    },
    {
      "category": "Maintainability",
      "severity": "minor",
      "file": "src/mcp/tools/push.ts",
      "line": 9,
      "message": "JSON.parse in handlePush is unguarded. A malformed payload string will throw a raw SyntaxError that bypasses the outer try-catch in create-server.ts, surfacing an unformatted error to the MCP caller",
      "suggestion": "Wrap the JSON.parse call in a try-catch inside handlePush and return a toolError with a descriptive message on parse failure",
      "effort": "trivial",
      "theme": "complexity",
      "evidence": "export async function handlePush(payloadJson: string): Promise<ToolResult> {\n  const raw = JSON.parse(payloadJson) as unknown;\n  const validated = pushSchema.parse(raw);",
      "scoreImpact": 1.5,
      "effortHours": 0.5,
      "ruleId": "CQ-009",
      "source": "llm"
    },
    {
      "category": "Documentation",
      "severity": "minor",
      "file": "src/client.ts",
      "line": 1,
      "message": "NoTicketsClient is a public class with no JSDoc on its methods. The class has a brief module-level comment but individual methods (push, connect, status, taskUpdate) lack documentation explaining their contracts, error behaviour, or relationship to the v2 api-client",
      "suggestion": "Add JSDoc to each public method describing the HTTP endpoint called, the shape of the return value, and when the promise rejects vs resolves with a failure object",
      "effort": "small",
      "theme": "missing-docs",
      "evidence": "export class NoTicketsClient {\n  private readonly config: SyncConfig;\n\n  constructor(config: SyncConfig) {\n    this.config = config;\n  }\n\n  async push(snapshot: StateSnapshot): Promise<PushResult> {",
      "scoreImpact": 1,
      "effortHours": 1,
      "ruleId": "CQ-010",
      "source": "llm"
    },
    {
      "category": "Documentation",
      "severity": "info",
      "file": "src/commands/task-update.ts",
      "line": 1,
      "message": "The module exports TaskUpdateData, VALID_STATUSES, VALID_PHASES, and parsing functions but has no module-level comment explaining its role or how it relates to the CLI task-update subcommand",
      "suggestion": "Add a brief module-level comment (similar to the pattern in src/core/index.ts) describing the module responsibility and the relationship between parseTaskUpdateArgs and the CLI entry point",
      "effort": "trivial",
      "theme": "missing-docs",
      "evidence": "export const VALID_STATUSES = ['not_started', 'in_progress', 'completed'] as const;\nexport const VALID_PHASES = ['red', 'green', 'refactor', 'review', 'complete'] as const;",
      "scoreImpact": 0.5,
      "effortHours": 0.25,
      "ruleId": "CQ-011",
      "source": "llm"
    },
    {
      "category": "Maintainability",
      "severity": "info",
      "file": "src/agent-detect.ts",
      "line": 57,
      "message": "agentType values 'agent' and 'human' are cast with 'as AssigneeType' on lines 57 and 67 even though the string literals already satisfy the AssigneeType union without an assertion",
      "suggestion": "Remove the 'as AssigneeType' casts — TypeScript infers the correct union member from the string literal without an explicit assertion, and the cast would hide type errors if AssigneeType changes",
      "effort": "trivial",
      "theme": "type-safety",
      "evidence": "return {\n  agent: check.agent,\n  agentType: 'agent' as AssigneeType,\n  vendor: check.vendor,\n  environment,\n};",
      "scoreImpact": 0.5,
      "effortHours": 0.25,
      "ruleId": "CQ-012",
      "source": "llm"
    },
    {
      "category": "Performance",
      "severity": "major",
      "file": "src/cli.ts",
      "line": 36,
      "message": "Sequential async I/O in readNoTicketsDir: stat() and readFile() are awaited one at a time in a for loop, serialising all filesystem reads",
      "suggestion": "Collect all top-level items then fan out with Promise.all — one Promise.all for stat calls, then another for readFile calls on matched items. The same pattern applies to subdirectory reads.",
      "effort": "small",
      "theme": "blocking-io",
      "evidence": "for (const item of items) {\n  const itemStat = await stat(itemPath).catch(() => null);\n  ...\n  const content = await readFile(itemPath, utf-8);\n  entries.push({ path: itemPath, content });\n}",
      "scoreImpact": 8,
      "effortHours": 1,
      "ruleId": "PERF-001",
      "source": "llm"
    },
    {
      "category": "Performance",
      "severity": "major",
      "file": "src/mcp/tools/validate.ts",
      "line": 16,
      "message": "readNoTicketsDir is a verbatim copy of the same sequential-I/O function from src/cli.ts, duplicating both the bottleneck and the maintenance surface",
      "suggestion": "Extract readNoTicketsDir into a shared utility (e.g. src/core/fs.ts) and parallelise the I/O there with Promise.all. Both callers benefit from a single fix.",
      "effort": "small",
      "theme": "blocking-io",
      "evidence": "for (const item of items) {\n  const itemStat = await stat(itemPath).catch(() => null);\n  ...\n  const content = await readFile(itemPath, utf-8);\n  entries.push({ path: itemPath, content });\n}",
      "scoreImpact": 8,
      "effortHours": 1,
      "ruleId": "PERF-002",
      "source": "llm"
    },
    {
      "category": "Performance",
      "severity": "minor",
      "file": "src/core/diff.ts",
      "line": 103,
      "message": "JSON.stringify used for deep equality of the optional meta field on every feature comparison. Serialising arbitrary objects on every diffFeature call is unnecessarily expensive when meta is absent or shallow.",
      "suggestion": "Guard with a fast-path: if both values are undefined or strictly equal, skip the stringify. For non-trivial meta objects a recursive structural equality helper avoids serialisation overhead entirely.",
      "effort": "trivial",
      "theme": "algorithm",
      "evidence": "if (JSON.stringify(prev.meta) \\!== JSON.stringify(curr.meta)) {\n  changes[meta] = { from: prev.meta, to: curr.meta };\n}",
      "scoreImpact": 3,
      "effortHours": 0.5,
      "ruleId": "PERF-003",
      "source": "llm"
    },
    {
      "category": "Performance",
      "severity": "minor",
      "file": "src/core/state.ts",
      "line": 25,
      "message": "Array.filter() allocates a transient array solely to obtain its length when counting completed tasks during feature state construction",
      "suggestion": "Replace with a reduce or a simple for-loop counter: feature.tasks.reduce((n, t) => t.status === completed ? n + 1 : n, 0). This avoids the intermediate array allocation.",
      "effort": "trivial",
      "theme": "algorithm",
      "evidence": "tasks: {\n  total: feature.tasks.length,\n  completed: feature.tasks.filter((t) => t.status === completed).length,\n},",
      "scoreImpact": 2,
      "effortHours": 0.25,
      "ruleId": "PERF-004",
      "source": "llm"
    },
    {
      "category": "Performance",
      "severity": "minor",
      "file": "src/cli.ts",
      "line": 62,
      "message": "readStdin wraps each incoming chunk in Buffer.from() before pushing it, creating an unnecessary copy of every chunk. The original chunk is already a Buffer-compatible Uint8Array.",
      "suggestion": "Push the raw chunk directly: chunks.push(chunk as Buffer). Buffer.concat accepts Uint8Array items, so the intermediate copy is not needed.",
      "effort": "trivial",
      "theme": "memory-leak",
      "evidence": "for await (const chunk of process.stdin) {\n  chunks.push(Buffer.from(chunk as Uint8Array));\n}\nreturn Buffer.concat(chunks).toString(utf-8);",
      "scoreImpact": 1,
      "effortHours": 0.25,
      "ruleId": "PERF-005",
      "source": "llm"
    },
    {
      "category": "Security",
      "severity": "major",
      "file": "src/mcp/tools/validate.ts",
      "line": 41,
      "message": "Path traversal risk: MCP validate tool accepts arbitrary directory path from untrusted input without sanitization. An MCP client could pass paths like \"../../../etc\" or absolute paths to read files outside the project.",
      "suggestion": "Resolve the directory argument and verify it is a descendant of the current working directory using path.resolve() and a startsWith check against a trusted base. Reject absolute paths and paths containing \"..\" segments.",
      "effort": "small",
      "theme": "input-validation",
      "evidence": "export async function handleValidate(directory?: string): Promise<ToolResult> {\n  const dir = directory ?? \".notickets\";\n  const files = await readNoTicketsDir(dir);\n  const result = validateFiles(files);\n  return toolSuccess(result);\n}",
      "references": [
        "CWE-22"
      ],
      "scoreImpact": 8,
      "effortHours": 1,
      "ruleId": "SEC-001",
      "source": "llm"
    },
    {
      "category": "Security",
      "severity": "major",
      "file": "src/sdk/credentials.ts",
      "line": 30,
      "message": "Credentials directory created without restricted permissions. The ~/.notickets directory is created with default umask permissions (typically 0o755), allowing other users on a shared system to list its contents and discover that credentials exist.",
      "suggestion": "Create the directory with mode 0o700 to restrict access to the owning user only: fs.mkdirSync(dir, { recursive: true, mode: 0o700 }). This provides defense-in-depth alongside the file-level 0o600 permission already applied.",
      "effort": "trivial",
      "theme": "secrets-management",
      "evidence": "const dir = credentialsDir();\nif (\\!fs.existsSync(dir)) {\n  fs.mkdirSync(dir, { recursive: true });\n}\n\nconst credentials: StoredCredentials = { token, email, expiresAt };\nconst filePath = credentialsPath();\nfs.writeFileSync(filePath, JSON.stringify(credentials, null, 2), \"utf-8\");",
      "references": [
        "CWE-276"
      ],
      "scoreImpact": 5,
      "effortHours": 0.25,
      "ruleId": "SEC-002",
      "source": "llm"
    },
    {
      "category": "Security",
      "severity": "major",
      "file": "src/sdk/auth-server.ts",
      "line": 30,
      "message": "Local OAuth callback server accepts tokens over plain HTTP without CSRF protection. Any local process or website can send a crafted request to http://127.0.0.1:<port>/callback?token=<malicious_token> to inject an attacker-controlled token, completing a session fixation attack.",
      "suggestion": "Generate a cryptographically random state parameter when starting the auth flow, pass it to the auth URL, and validate that the callback includes the matching state parameter before accepting the token. This prevents cross-site and cross-process token injection.",
      "effort": "medium",
      "theme": "auth-hardening",
      "evidence": "const server = http.createServer((req, res) => {\n  const url = new URL(req.url ?? \"/\", `http://127.0.0.1`);\n  if (url.pathname \\!== \"/callback\") {\n    res.writeHead(404);\n    res.end();\n    return;\n  }\n  const token = url.searchParams.get(\"token\");",
      "references": [
        "CWE-352",
        "CWE-384"
      ],
      "scoreImpact": 8,
      "effortHours": 4,
      "ruleId": "SEC-003",
      "source": "llm"
    },
    {
      "category": "Security",
      "severity": "minor",
      "file": "src/sdk/api-client.ts",
      "line": 71,
      "message": "URL construction via string concatenation without validation. The apiUrl is concatenated directly with path strings, and the path parameters (e.g., projectId, featureId) are interpolated without encoding. A malicious or malformed projectId containing slashes or special URL characters could alter the intended API endpoint.",
      "suggestion": "Use URL constructor to safely compose URLs: new URL(path, apiUrl). Validate or encode user-provided path segments using encodeURIComponent() to prevent path manipulation.",
      "effort": "small",
      "theme": "input-validation",
      "evidence": "async function request(apiUrl: string, token: string, path: string, options?: RequestInit): Promise<unknown> {\n  const url = `${apiUrl}${path}`;",
      "references": [
        "CWE-20"
      ],
      "scoreImpact": 4,
      "effortHours": 1,
      "ruleId": "SEC-004",
      "source": "llm"
    },
    {
      "category": "Security",
      "severity": "minor",
      "file": "src/cli.ts",
      "line": 156,
      "message": "Unsanitized user input reflected in error output. The first CLI argument is echoed directly to stderr without sanitization. While this is a CLI tool (not a web context), if stderr is consumed by another process or rendered in a terminal with escape sequence support, this could enable terminal injection.",
      "suggestion": "Sanitize the argv[0] value by stripping control characters before including it in the error message, or use a fixed message that does not reflect user input verbatim.",
      "effort": "trivial",
      "theme": "input-validation",
      "evidence": "case \"unknown\":\n  console.error(`Unknown command: ${argv[0]}\\nRun \"npx no-tickets --help\" for usage.`);\n  process.exitCode = 1;\n  break;",
      "references": [
        "CWE-117"
      ],
      "scoreImpact": 2,
      "effortHours": 0.25,
      "ruleId": "SEC-005",
      "source": "llm"
    },
    {
      "category": "Security",
      "severity": "minor",
      "file": "src/sdk/api-client.ts",
      "line": 93,
      "message": "Error responses may leak sensitive information. When the API returns a non-OK response with a JSON error body, the error message from the server is included directly in the thrown Error. If error messages contain internal server details, stack traces, or token information, these could be exposed to callers or logged.",
      "suggestion": "Consider sanitizing or truncating server error messages before including them in thrown Errors. At minimum, limit the length of the error string and avoid including headers or request details in error messages.",
      "effort": "trivial",
      "theme": "data-exposure",
      "evidence": "if (\\!response.ok) {\n  const message = hasErrorField(body)\n    ? String(body.error)\n    : \"Request failed\";\n  throw new Error(`${response.status}: ${message}`);\n}",
      "references": [
        "CWE-209"
      ],
      "scoreImpact": 2,
      "effortHours": 0.5,
      "ruleId": "SEC-006",
      "source": "llm"
    },
    {
      "category": "Security",
      "severity": "minor",
      "file": "src/cli.ts",
      "line": 27,
      "message": "Directory traversal in readNoTicketsDir allows reading markdown files from arbitrary subdirectory depth. While currently hardcoded to .notickets, the function only limits to two levels of depth but performs no path canonicalization. The duplicate implementation in src/mcp/tools/validate.ts shares this pattern.",
      "suggestion": "Add a canonicalization step using path.resolve() and ensure the resolved base directory is within the expected project root. Consider extracting this shared function into a single module to eliminate the DRY violation which doubles the attack surface.",
      "effort": "small",
      "theme": "input-validation",
      "evidence": "async function readNoTicketsDir(dir: string): Promise<readonly FileEntry[]> {\n  const entries: FileEntry[] = [];\n  let items: string[];\n  try {\n    items = await readdir(dir);\n  } catch {\n    return [];\n  }",
      "references": [
        "CWE-22"
      ],
      "scoreImpact": 3,
      "effortHours": 1.5,
      "ruleId": "SEC-007",
      "source": "llm"
    },
    {
      "category": "Security",
      "severity": "info",
      "file": "src/mcp/tools/push.ts",
      "line": 9,
      "message": "JSON.parse of untrusted MCP input without try/catch. If malformed JSON is passed as payloadJson, the unhandled exception will propagate. While the caller in create-server.ts has a try/catch, defense-in-depth suggests validating closer to the untrusted input boundary.",
      "suggestion": "Wrap JSON.parse in a try/catch and return a descriptive ToolResult error rather than relying on the outer catch handler. This provides clearer error messages for MCP clients and follows defense-in-depth principles.",
      "effort": "trivial",
      "theme": "input-validation",
      "evidence": "export async function handlePush(payloadJson: string): Promise<ToolResult> {\n  const raw = JSON.parse(payloadJson) as unknown;\n  const validated = pushSchema.parse(raw);",
      "references": [
        "CWE-20"
      ],
      "scoreImpact": 1,
      "effortHours": 0.25,
      "ruleId": "SEC-008",
      "source": "llm"
    },
    {
      "category": "Testing",
      "severity": "major",
      "file": "src/sdk/__tests__/credentials.test.ts",
      "line": 138,
      "message": "vi.useFakeTimers() restored inline with vi.useRealTimers() inside the test body rather than in afterEach, so if the test throws before reaching line 152 the fake timer state leaks into subsequent tests",
      "suggestion": "Move vi.useFakeTimers() setup to beforeEach (or the start of the test) and move vi.useRealTimers() into afterEach so cleanup is guaranteed on test failure",
      "effort": "trivial",
      "theme": "flaky-tests",
      "evidence": "vi.useFakeTimers();\n    vi.setSystemTime(new Date(\"2026-06-01T12:00:00Z\"));\n    ...\n    expect(result).toBeNull();\n    vi.useRealTimers(); // only reached if test passes",
      "scoreImpact": 3,
      "effortHours": 0.25,
      "ruleId": "TEST-flaky-fake-timers",
      "source": "llm"
    },
    {
      "category": "Testing",
      "severity": "major",
      "file": "src/sdk/__tests__/auth-server.test.ts",
      "line": 119,
      "message": "Real timing delay of 150 ms used to verify promise stability after a short timeout fires — introduces wall-clock dependency that can slow the suite and fail on slow CI runners",
      "suggestion": "Use vi.advanceTimersByTimeAsync() with fake timers, or restructure the test to assert the already-resolved promise value without waiting for real elapsed time",
      "effort": "small",
      "theme": "flaky-tests",
      "evidence": "// Wait longer than timeout to confirm promise stays resolved\n    await new Promise((r) => setTimeout(r, 150));",
      "scoreImpact": 3.5,
      "effortHours": 0.5,
      "ruleId": "TEST-flaky-timing",
      "source": "llm"
    },
    {
      "category": "Testing",
      "severity": "major",
      "file": "src/sdk/__tests__/api-client.test.ts",
      "line": 5,
      "message": "Module-level let fetchSpy is reassigned in beforeEach but vi.clearAllMocks() is never called — only vi.restoreAllMocks() runs in afterEach. If a test sets a persistent mockReturnValue (not mockReturnValueOnce), the return value can bleed into the next test within the same describe block",
      "suggestion": "Add vi.clearAllMocks() to the beforeEach block alongside the fetchSpy reassignment to guarantee mock state is wiped before every test",
      "effort": "trivial",
      "theme": "flaky-tests",
      "evidence": "let fetchSpy: ReturnType<typeof vi.fn>;\n\nbeforeEach(() => {\n  fetchSpy = vi.fn();\n  vi.stubGlobal(\"fetch\", fetchSpy);\n});\n\nafterEach(() => {\n  vi.restoreAllMocks(); // clearAllMocks missing\n});",
      "scoreImpact": 2.5,
      "effortHours": 0.1,
      "ruleId": "TEST-missing-mock-cleanup",
      "source": "llm"
    },
    {
      "category": "Testing",
      "severity": "major",
      "file": "src/sdk/__tests__/auth-server.test.ts",
      "line": 1,
      "message": "auth-server.test.ts starts a real HTTP server and makes real network requests (fetch to 127.0.0.1) — it is an integration test mixed into the same test run as pure unit tests with no way to run them separately",
      "suggestion": "Tag or separate integration tests using vitest workspaces or a separate vitest config (e.g. vitest.integration.config.ts) so developers can run only fast unit tests during the edit-save loop",
      "effort": "medium",
      "theme": "test-organisation",
      "evidence": "const { port, tokenPromise, close } = await startAuthServer();\ncleanup = close;\n\nconst response = await fetch(`http://127.0.0.1:${port}/callback?token=nt_session_test123`);",
      "scoreImpact": 4,
      "effortHours": 2,
      "ruleId": "TEST-organisation-integration",
      "source": "llm"
    },
    {
      "category": "Testing",
      "severity": "minor",
      "file": "src/commands/task-update.test.ts",
      "line": 1,
      "message": "Test file is co-located directly alongside the source file (src/commands/task-update.test.ts) rather than in the __tests__/ subdirectory used by every other test in the project",
      "suggestion": "Move to src/commands/__tests__/task-update.test.ts to follow the established project convention and make test files easy to locate",
      "effort": "trivial",
      "theme": "test-organisation",
      "evidence": "// File location: src/commands/task-update.test.ts\n// All other command tests live under src/commands/__tests__/\n// e.g. src/commands/__tests__/token.test.ts",
      "scoreImpact": 1,
      "effortHours": 0.1,
      "ruleId": "TEST-organisation-convention",
      "source": "llm"
    },
    {
      "category": "Testing",
      "severity": "minor",
      "file": "src/core/__tests__/templates.test.ts",
      "line": 11,
      "message": "readFileSync is called against real files on disk (the templates/ directory) — these tests will fail if the working directory changes or template files are moved, and they cannot run in environments without the full repo checkout",
      "suggestion": "This is an acceptable trade-off for template conformance tests, but add a descriptive comment explaining the real-I/O dependency and ensure the file paths are resolved relative to import.meta.url (already done). Consider labelling these as integration tests if the project separates categories.",
      "effort": "trivial",
      "theme": "flaky-tests",
      "evidence": "function readTemplate(name: string): string {\n  return readFileSync(join(TEMPLATES_DIR, name), \"utf-8\");\n}",
      "scoreImpact": 1,
      "effortHours": 0.25,
      "ruleId": "TEST-flaky-real-io",
      "source": "llm"
    },
    {
      "category": "Testing",
      "severity": "minor",
      "file": "vitest.config.ts",
      "line": 1,
      "message": "No test category separation configured — all 23 test files (including real-HTTP integration tests in auth-server.test.ts) run as a single undifferentiated batch. There is no mechanism to run only fast unit tests during development.",
      "suggestion": "Add vitest workspaces or use project config to separate unit tests (<5 s) from integration tests. At minimum, add a test:unit script that excludes auth-server.test.ts using --testPathPattern, so the edit-save feedback loop stays fast.",
      "effort": "small",
      "theme": "test-organisation",
      "evidence": "export default defineConfig({\n  test: {\n    coverage: { provider: \"v8\", include: [\"src/**/*.ts\"] },\n  },\n});",
      "scoreImpact": 2,
      "effortHours": 1,
      "ruleId": "TEST-organisation-categories",
      "source": "llm"
    }
  ]
}
```
