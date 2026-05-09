---
id: publish-shared-surfaces
type: fix
title: Wire CLI publish, share local validation between CLI/MCP via SDK, unify project-keyed shape
phase: development
status: not_started
severity: medium
created: 2026-05-09
updated: 2026-05-09
reported: 2026-05-09T00:00:00.000Z
resolved: null
---

# Fix: Wire CLI publish, share local validation between CLI/MCP via SDK, unify project-keyed shape

## Phase context (4-phase client roadmap)

This fix is **Phase 1** of a four-phase plan for the no-tickets client surface.

### Rationale

- **Phase 1 — fix the TS surface.** Need a working baseline for CLI publish, shared validation, project registry, and the schema-distribution model before doing anything ambitious.
- **Phase 2/3 — Rust binary, distributed everywhere.** CLI and MCP shouldn't require a Node runtime. After this, *everyone* (any language, any environment) gets first-class CLI/MCP via a single binary distributed through every relevant package manager.
- **Post-Phase-3 steady state**: everyone uses the Rust binary for CLI/MCP. TS users *additionally* have the programmatic SDK from Phase 1. Python/Go/Rust/etc. users in this window either shell out to the binary from their code, or hit the HTTP endpoint directly.
- **Phase 4 — per-language SDKs on demand.** When concrete adoption of (e.g.) Python or Go shows up, that language gets a thin SDK matching the TS SDK's shape. The TS SDK becomes one of three peers, not the only option. Don't pre-build them.

### Phase-by-phase user journey by language

| Surface used | After Phase 1 | After Phase 3 | After Phase 4 |
|---|---|---|---|
| **CLI** (`nt publish ...`) | TS-via-npm CLI | Rust binary (every package manager) | Rust binary (unchanged) |
| **MCP server** (agent tool calls) | TS server via npm | Rust binary | Rust binary (unchanged) |
| **Programmatic — TS** (`import { publish }`) | TS SDK | TS SDK (carried over) | Thin TS SDK (matches Python/Go shape) |
| **Programmatic — Python** | None (use CLI or raw HTTP) | None (use CLI or raw HTTP) | Thin Python SDK *if demanded* |
| **Programmatic — Go** | None (use CLI or raw HTTP) | None (use CLI or raw HTTP) | Thin Go SDK *if demanded* |

### Phase dependencies

| Phase | Fix | What lands | Depends on |
|---|---|---|---|
| **1 (this fix)** | `publish-shared-surfaces.md` | TS CLI `publish` wired, validation in SDK shared by CLI/MCP, project registry, `--token` / `--token-stdin` / `--token-env-var` shape, drop CI auto-detection, npm-bundled Zod schemas as source of truth | — |
| **2** | `cross-platform-cli-binary.md` | Full Rust rewrite of CLI + MCP server. Validates against the JSON Schema build artifact from `no-tickets-service`. TS CLI and MCP code retired. | Phase 1 (defines the surface to port) |
| **3** | (same fix as Phase 2) | Multi-channel distribution: cargo, Homebrew, Scoop, deb/rpm, npm wrapper (for transparent migration of current `npx no-tickets` users), install script | Phase 2 |
| **4** | future fix | Thin TS / Python / Go SDKs — same shape across languages. TS package post-Phase 4 = SDK only (CLI/MCP have moved to Rust). | Phase 3 (stable wire/binary contract) — though strictly only the wire contract is needed; can run in parallel if pressing |

Phase 1 ships first because Phase 2 needs a concrete surface to port. 2 unlocks 3 (the binary needs to exist before it can be distributed). 4 can run in parallel with 2/3 if there's enough engineering capacity, but practically benefits from a stable wire/schema contract underneath.

## Issue Summary

**Reported:** 2026-05-09
**Severity:** medium

Four problems converge:

1. **`nt publish` is unwired.** The dispatcher in `src/cli.ts` falls through to `default` ("Command 'publish' is not yet implemented") even though `runPublishSingle` (`src/cli/commands/publish/single.ts`) is fully implemented and tested. The smoke script (`scripts/smoke-publish.ts`) is the only working command-line publish path and is dev-only.

2. **MCP `publish_event` has no local validation.** `src/mcp/tools/handlers.ts:122-149` builds a `PublishEvent` and POSTs it; invalid payloads round-trip to the server before being rejected. The CLI does pre-flight schema validation (`src/cli/lib/schema-validate.ts`), but MCP doesn't — so agents get slower feedback and waste server cycles.

3. **Local validation lives in the CLI layer.** It should live in the SDK so MCP, CLI, and any future SDK consumer call one canonical implementation. `src/cli/lib/schema-validate.ts` is CLI-coupled today.

4. **Schema source-of-truth on the client is implicit + fragile.** Today, validation needs an `EventTypeSpec` fetched per-publish (or per-session for MCP) from `/v1/admin/event-types/{id}`. That couples publish to a network round-trip, requires cache-invalidation reasoning, and silently mismatches when a server schema mutates within the same `.vN` id. The client has no audit trail of *what schemas it validated against*.

Additionally, neither CLI nor MCP can address multiple projects on one machine. CI needs `npx no-tickets publish someProject <data>` and `npx no-tickets publish otherProject <data>` from the same job — today's single `NO_TICKETS_TOKEN` env var can't model that. A project registry is a prerequisite for a coherent `publish <project> <payload>` shape on both surfaces.

### Schema-distribution decision: npm-bundled (hybrid)

After exploring three architectures (per-publish fetch, init-time pull, npm-bundled package), we settled on the **hybrid npm-bundled** model:

```
no-tickets-service (server repo)
  packages/schemas/  ── Zod source of truth
       │
       ├─► server runtime (validate inbound events)
       ├─► /v1/registry/event-types HTTP endpoint (Zod → JSON Schema, served with ETag)
       └─► auto-publish to npm: @magic-ingredients/no-tickets-schemas

This SDK (no-tickets, where we are now)
  └─► imports @magic-ingredients/no-tickets-schemas as a runtime dependency
  └─► validateEventLocally reads from bundled Zod schemas — no network
  └─► getEventType / listEventTypes HTTP code paths kept (used by future non-JS SDKs and by inspection commands like `nt registry list`)

Future Go / Python / Rust SDKs
  └─► fetch from /v1/registry/event-types, cache locally, use idiomatic schema lib
```

**Why npm-bundled for JS:**
- Lock-file integrity (cryptographic hash) and dependabot/renovate work natively
- No network at publish time; offline / air-gapped CI works trivially
- Static `import` gives consumers `event.data` typing for free via Zod inference
- Adding a new event type means `npm update @magic-ingredients/no-tickets-schemas` — a normal dependency PR, reviewable in the same flow as other deps
- Server-side schema mutation can't silently change client behavior — the client is pinned to a specific schemas package version

**Why also keep the HTTP endpoint:**
- Future non-JS clients (Go, Python, Rust) need a uniform contract — the HTTP types endpoint is that contract
- No commitment to per-language packaging until a language gets enough demand to justify it
- When a language gets a "real" SDK, the maintainers can choose runtime fetch vs install-time codegen — implementation detail per language

**Why not init-time pull** (rejected): builds custom infrastructure (registry-pull command, local persistence file, server `?version=X` endpoint) that npm + GitHub Releases already give you for free.

**Why not server-side fetch only** (rejected): regresses the JS DX — loses type imports, loses lock-file integrity, requires per-instance fetch+cache code, blocks offline CI.

**Why not per-language packages now** (deferred): premature optimization. PyPI/crates.io/Hex publishing pipelines are heavy; build them when there's adoption signal, not on speculation.

### Target shape

**CLI — call shapes:**

```
# Project-keyed (local dev — reads ~/.notickets/config.json)
npx no-tickets publish <project> <event-json>
echo '<event-json>' | npx no-tickets publish <project> -

# CI multi-project (caller names its own env vars per project)
npx no-tickets publish --token-env-var NT_TOKEN_PROJECT_A --url <api-url> <event-json>
npx no-tickets publish --token-env-var NT_TOKEN_PROJECT_B --url <api-url> <event-json>

# One-off override (debug / explicit testing — token visible on argv, prefer --token-stdin)
npx no-tickets publish --token <token> --url <api-url> <event-json>
echo "$TOKEN" | npx no-tickets publish --token-stdin --url <api-url> <event-json>

# Env-var legacy (single-project CI; unchanged from today)
NO_TICKETS_TOKEN=... NO_TICKETS_API_URL=... npx no-tickets publish <event-json>
```

**MCP (named, flat):**
```
publish_event({ project, type, data, subject?, occurred_at?, parent_event_id?, trace_id?, dedupe_key?, source? })
```

MCP optionally accepts `token` + `api_url` for parity with CLI override path (rarely useful for agents).

`<event-json>` is the full envelope: `{ type, data, subject?, occurredAt?, parentEventId?, traceId?, dedupeKey?, source? }` (camelCase to match SDK).

### Resolution precedence

For each `publish` call:
```
token = --token | --token-stdin > --token-env-var <NAME> > NO_TICKETS_TOKEN env > project-lookup
url   = --url > NO_TICKETS_API_URL env > profile-via-project | default
```

Token-source flags are mutually exclusive; passing more than one of `--token` / `--token-stdin` / `--token-env-var` is an error. Each independently falls back through the URL precedence (project positional + `--url` is a valid combo: token from registry, URL overridden).

### Security note

`--token` puts the secret on argv (visible in `ps`, shell history, CI logs). Recommended overrides:
- **`--token-env-var <NAME>`** — caller names which env var holds the token; secret never appears on argv. Best for CI multi-project.
- **`--token-stdin`** — read from stdin (Docker pattern). Best for piping from a secret manager.

`nt publish --help` surfaces this trade-off.

### Source provenance — explicit, not detected

Today `src/agent-detect.ts` reads `GITHUB_ACTIONS` / `GITLAB_CI` / etc. env vars and silently stamps `source.name = 'ci'` on every event. This is brittle:

- Self-hosted runners that don't set the standard env var → mislabeled as `sdk`
- Devcontainers / `act` / dev shells with `GITHUB_ACTIONS=true` set → local work silently labeled `ci`
- New providers / renamed env vars → never updated

This fix removes auto-detection. Defaults become surface-specific:
- CLI publish → `source: { name: 'cli', sdkVersion }`
- MCP publish → `source: { name: 'mcp', sdkVersion }` (already)
- Direct SDK → `source: { name: 'sdk', sdkVersion }`

Caller-driven provenance is unchanged — `PublishEvent.source: Partial<Source>` already exists and merges over the default. CI scripts that want CI provenance set it explicitly:

```
nt publish myapp \
  --source-attribute provider=github-actions \
  --source-attribute runId="$GITHUB_RUN_ID" \
  '<event-json>'
```

Or via the event envelope (`source.attributes.provider`).

The machine-hash feature (`NO_TICKETS_INCLUDE_MACHINE=1`) stays — it's already explicit opt-in.

## Root Cause

- **Unwired publish:** `src/cli.ts:27,41` recognises `'publish'` for parser/help purposes but the switch-case in `runCli` doesn't dispatch to `runPublishSingle`. Carryover from staged feature implementation that was never integrated.
- **MCP no-validation:** `handlePublishEvent` was built before the CLI's local-validation utility existed; the SDK never grew a public validator, so MCP had nothing to call.
- **Validation in CLI layer:** `src/cli/lib/schema-validate.ts` was scoped to CLI as a UX helper; pushing it into the SDK was out-of-scope at the time.
- **No project registry:** the `--profile` mechanism models environments (URL pairs) but not project tokens. Push tokens currently flow only via `NO_TICKETS_TOKEN` env, which is single-valued.
- **No bundled schemas:** the only way to know what schemas exist is to ask the server. There's no compile-time / install-time source of truth on the client.

### Affected Files

**SDK (additions):**
- `src/transport/validate.ts` (new) — public `validateEventLocally(event)` reading from bundled Zod schemas; returns `ValidationIssue[]`
- `src/transport/index.ts` — export the validator
- `src/sdk/projects.ts` (new) — load/parse `projects` section from `~/.notickets/config.json`
- `src/sdk/url-resolver.ts` — extend to resolve `{ apiUrl, authUrl, pushToken }` when given a project name
- `package.json` — add `@magic-ingredients/no-tickets-schemas` as a runtime dependency (pre-req: server-side publishing pipeline lands first; coordinate with no-tickets-service team)

**CLI:**
- `src/cli.ts` — wire `publish` case → `runPublishSingle`; add `project` (project|link|list|unlink) command tree
- `src/cli/commands/publish/single.ts` — replace local `validateAgainstSchema` import with SDK's validator; accept `project` positional; resolve token from registry; accept `--token`, `--token-stdin`, `--token-env-var`, `--url`
- `src/cli/lib/schema-validate.ts` — delete; CLI imports SDK's `validateEventLocally`
- `src/cli/commands/project/link.ts` (new), `list.ts` (new), `unlink.ts` (new)

**MCP:**
- `src/mcp/tools/handlers.ts` — `handlePublishEvent` adds local validation step using the bundled schemas package; reject invalid payloads before `publishEvents`
- `src/mcp/tools/publish-event.ts` — add `project` field to input schema
- MCP transport setup — resolve token from project registry

**Source-detection cleanup:**
- `src/agent-detect.ts` — strip CI-provider sniffing; keep machine-hash helper

**Smoke script:**
- `scripts/smoke-publish.ts` — accept `--project` and `--token-env-var`

## Test Plan

### Regression Tests (must pass unchanged)
| File | Cases | Status |
|------|-------|--------|
| `src/transport/events.test.ts` | all existing | - |
| `src/cli/commands/publish/single.test.ts` | all existing (after refactor to SDK validator) | - |
| `src/cli/commands/publish/batch.test.ts` | all existing | - |
| `src/mcp/tools/handlers.test.ts` | all existing publish_event cases | - |
| `src/sdk/__tests__/auth-server.test.ts` | all existing | - |

### New Tests (to be added)
| File | Case | Status |
|------|------|--------|
| `src/transport/__tests__/validate.test.ts` | `validateEventLocally` happy path + each error shape | - |
| `src/sdk/__tests__/projects.test.ts` | parse config.json projects; parse NO_TICKETS_CONFIG_JSON; precedence; missing project errors | - |
| `src/cli/commands/project/link.test.ts` | link writes 0600 config; rejects duplicate without --force | - |
| `src/cli/commands/project/list.test.ts` | masks tokens; shows env name | - |
| `src/cli/commands/project/unlink.test.ts` | removes entry; idempotent | - |
| `src/__tests__/publish-cli-e2e.test.ts` | `nt publish <project> <json>` resolves token, validates locally, calls publish | - |
| `src/mcp/tools/handlers.test.ts` | publish_event rejects invalid payload before HTTP call; resolves project token | - |

## Tasks

### 1. SDK — extract local validation into `validateEventLocally`
status: not_started

Move `src/cli/lib/schema-validate.ts` logic into `src/transport/validate.ts` and export from the SDK barrel. Signature:

```ts
export interface ValidationIssue { readonly path: string; readonly message: string; }
export function validateEventLocally(
  data: unknown,
  typeSpec: { readonly schema: JsonSchema },
): readonly ValidationIssue[];
```

CLI and MCP both depend on this. Tests cover happy path + each Zod/JSON-schema rejection shape.

**Files to modify/create:**
- `src/transport/validate.ts` (new)
- `src/transport/__tests__/validate.test.ts` (new)
- `src/transport/index.ts` (export)
- `src/cli/lib/schema-validate.ts` (delete after callers migrate)

### 2. SDK — project registry loader + `clientForProject` factory
status: not_started

Consistent with the existing pattern (`src/sdk/auth.ts`, `credentials.ts`, `url-resolver.ts` are all programmatic config-readers), add:

**Lower-level helper** — `src/sdk/projects.ts`:
```ts
export function resolveProjectAuth(name: string): {
  readonly token: string;
  readonly apiUrl: string;
  readonly authUrl: string;
};
```

Resolution: read `~/.notickets/config.json` `projects[name]`; throw `ProjectNotRegisteredError(name, availableNames)` on miss. CI explicitly does NOT use this path — it uses `--token-env-var` instead, so no on-disk config is required in CI.

**Higher-level factory** — exported from the SDK barrel:
```ts
export function clientForProject(
  name: string,
  overrides?: Partial<ClientOptions>,
): Client;
```

So that CLI/MCP/smoke call sites collapse to one line:
```ts
const client = clientForProject('myapp');
await publish(client, [event]);
```

The lower-level `resolveProjectAuth` stays public for callers that want token+URL without a `Client` (e.g. `nt status --project myapp`).

`new Client({ baseUrl, token })` remains the primary path for production services that source secrets from elsewhere — `clientForProject` is purely an opt-in convenience for callers using the local config registry.

Schema additions to `config.json`:
```json
{
  "profiles": { ... },
  "projects": {
    "<name>": { "profile": "<profile-name>", "pushToken": "nt_push_..." }
  }
}
```

Project entry references a profile by name for URL resolution.

**Files to modify/create:**
- `src/sdk/projects.ts` (new) — `resolveProjectAuth`, `clientForProject`
- `src/sdk/__tests__/projects.test.ts` (new)
- `src/sdk/url-resolver.ts` (extend with project-aware overload)
- `src/transport/index.ts` or root barrel — export `clientForProject`

### 3. CLI — `project link/list/unlink` commands
status: not_started

Wire local-only project registry management. `link` writes a new entry to `~/.notickets/config.json` with file mode 0600; rejects duplicate names without `--force`. `list` masks tokens (`nt_push_...3f2`). `unlink` removes an entry.

Note: these commands are local-only — they never call the server. Push tokens are minted in the web UI and pasted into `link`.

**Files to modify/create:**
- `src/cli/commands/project/link.ts` (new) + tests
- `src/cli/commands/project/list.ts` (new) + tests
- `src/cli/commands/project/unlink.ts` (new) + tests
- `src/cli.ts` — dispatch `project` subcommand tree

### 4. CLI — wire `publish` and switch to SDK validator + project resolution
status: not_started

Update `runPublishSingle` to:
- Accept the three call shapes documented in "Target shape": positional project, explicit `--token`/`--url` (or `--token-stdin`), or env-var fallback
- Resolve token + URL via the precedence chain (token: `--token` > `--token-stdin` > env > project; url: `--url` > env > profile-via-project)
- Replace `validateAgainstSchema` import with SDK's `validateEventLocally`

Add dispatcher case in `src/cli.ts`. Keep fuzzy-match suggestions on unknown type id (CLI-only UX layer).

Shape: `nt publish <project> <event-json>` (positional JSON envelope) with stdin `-` form.

**Files to modify/create:**
- `src/cli.ts`
- `src/cli/commands/publish/single.ts`
- `src/cli/commands/publish/batch.ts` (parallel update)
- `src/cli/commands/publish/single.test.ts`, `batch.test.ts`
- `src/__tests__/publish-cli-e2e.test.ts` (new)

### 5. MCP — add local validation + `project` parameter
status: not_started

Update `handlePublishEvent` to:
- Accept `project` in args (added to `publishEventTool.inputSchema`)
- Resolve token via `resolveProjectAuth(project)` at the transport layer (or accept it via deps)
- Fetch the type spec (cached per MCP session) and call `validateEventLocally` before `publishEvents`
- Return typed validation errors to the agent (don't round-trip to server for shape errors)

**Files to modify/create:**
- `src/mcp/tools/publish-event.ts` (input schema)
- `src/mcp/tools/handlers.ts` (handler logic)
- `src/mcp/tools/handlers.test.ts`
- MCP transport setup (project-aware client construction)

### 6. Smoke script — accept `--project`, `--token-env-var`
status: not_started

`scripts/smoke-publish.ts` learns:
- `--project <name>` — resolves via `resolveProjectAuth`
- `--token-env-var <NAME>` — reads token from caller-named env var; pair with `--url`
The current `--profile` + `NO_TICKETS_TOKEN` path stays for back-compat.

**Files to modify/create:**
- `scripts/smoke-publish.ts`

### 7. Drop CI auto-detection from default source
status: not_started

Remove the CI-provider env-var sniffing from the default `Source` builder. Replace with surface-specific defaults:

- CLI publish handler → `source: { name: 'cli', sdkVersion }`
- MCP transport → `source: { name: 'mcp', sdkVersion }` (already correct)
- Direct SDK `Client` constructor → `source: { name: 'sdk', sdkVersion }`

Keep the `NO_TICKETS_INCLUDE_MACHINE` opt-in machine-hash code path. CI provenance becomes the caller's responsibility via `PublishEvent.source.attributes` (event envelope) or `--source-attribute key=val` (CLI flags, already implemented in `parseSourceFlags`).

`detectSource()` and `agent-detect.ts` simplify dramatically — possibly rename or delete. Callers that *want* the old behavior can build `source` themselves from `process.env.GITHUB_ACTIONS` etc.

**Files to modify/create:**
- `src/agent-detect.ts` — strip CI-provider sniffing; keep machine-hash helper
- `src/transport/client.ts` — simpler default source for direct SDK use
- `src/cli/commands/publish/single.ts` — default source for CLI surface
- `src/cli/commands/publish/single.test.ts`, `batch.test.ts` — update default-source assertions
- `src/agent-detect.test.ts` — drop CI-provider tests; keep machine-hash tests

## Acceptance Criteria

- [ ] `npx no-tickets publish <project> <event-json>` succeeds against staging given a registered project
- [ ] `npx no-tickets publish bogus '{}'` exits with `project not registered` and lists known projects
- [ ] `npx no-tickets publish <project> '{"type":"unknown.v1","data":{}}'` exits 2 with fuzzy-match suggestions, never POSTs
- [ ] `npx no-tickets publish <project> '{"type":"<known>","data":{<bad-shape>}}'` exits 1 with field-by-field validation errors, never POSTs
- [ ] MCP `publish_event` rejects malformed `data` locally, agent sees structured `ValidationIssue[]` without an HTTP round-trip
- [ ] CI: `NO_TICKETS_CONFIG_JSON='{...}' npx no-tickets publish <project> '...'` works with no on-disk config
- [ ] `nt project link/list/unlink` round-trip works; `link` enforces 0600 on `config.json`
- [ ] `src/cli/lib/schema-validate.ts` deleted; both CLI and MCP import from `src/transport/validate.ts`
- [ ] All regression tests pass

## Dependencies / Notes

- `validateEventLocally` operates on a `typeSpec` the caller has already fetched. Keeps SDK transport-agnostic for validation; registry fetch stays a separate concern.
- MCP type-spec cache should be invalidated when the registry signals a deprecation/change (out of scope for this fix).
- Project name format: alphanumeric + dash/underscore. No server validation — local nicknames only. Token still encodes server-side identity.
- `NO_TICKETS_TOKEN` env var path is preserved for legacy single-project CI; it's mutually exclusive with `--project` (passing both → error).
