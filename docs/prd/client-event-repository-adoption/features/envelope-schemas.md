---
id: envelope-schemas
prd_id: client-event-repository-adoption
number: 1
title: Envelope Schemas + SDK Surface Reset
status: completed
created: 2026-04-27
updated: 2026-05-17
---

# Feature: Envelope Schemas + SDK Surface Reset

## Description

Replace the current push-payload-shaped exports in `src/core/types.ts` and `src/core/schemas.ts` with envelope-only types: `Event`, `Source`, `Subject`, `Interaction`, `Session`, `Actor`, `SubjectRef`, plus the type-ID grammar parser. The SDK ships nothing about domain payloads — that's the server registry's job per ADR-0001.

This is the most disruptive feature in the PRD. Every existing consumer that imports `Push`, `WorkSchema`, `EngineeringSchema`, `ProductSchema`, `CodeQualitySchema`, etc. will fail to type-check after this lands. That's intentional: ADR-0001 and the no-v1-backcompat stance accept this as a clean cut.

### Wire-format types

```ts
type Event<T = unknown> = {
  readonly type: TypeId;
  readonly data: T;
  readonly source: Source;          // mandatory; SDK auto-fills based on entry surface
  readonly subject?: SubjectRef;
  readonly occurredAt?: string;
  readonly parentEventId?: string;
  readonly traceId?: string;
  readonly dedupeKey?: string;
};

type Source = {
  readonly name: string;            // 'cli' | 'mcp' | 'ci' | 'cron' | 'integration' | 'sdk'
  readonly sdkVersion: string;      // version of @magic-ingredients/no-tickets, SDK auto-fills
  readonly version?: string;        // version of the named producer (when distinct from SDK)
  readonly attributes?: Readonly<Record<string, string | number | boolean>>;
};

type SubjectRef = { readonly type: string; readonly id: string };

type Subject = {
  readonly type: string;
  readonly externalId: string;
  readonly displayName: string;
  readonly metadata?: Readonly<Record<string, unknown>>;
};

type Interaction<TInput = unknown> = {
  readonly id: string;
  readonly input: TInput;
  readonly subject?: SubjectRef;
};

// Type-ID grammar: ^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*){2}\.v[1-9]\d*$
type TypeId = string;
```

`Session`, `Actor`, and `PushEnvironment` survive but lose their push-specific fields and collapse into helpers that *construct* `Source` rather than sit alongside it as parallel envelope concepts. `detectAgent()` returns a `Source` directly (with appropriate `name` and `attributes` for detected CI providers, etc.).

### Source defaulting and override

`Source` is required on every event but rarely set explicitly. The SDK auto-fills `name` and `sdkVersion` based on entry surface (see PRD §"Source semantics" for the per-surface defaults). Caller-supplied `source` fields **merge** with auto-detected ones — caller wins on conflicts, but the auto-filled fields fill any gaps.

`attributes` is free-form `Record<string, string | number | boolean>`. The PRD documents conventions (cookbook) but the schema does not enforce them; callers can add their own keys freely.

### Refinement ban

Per ADR-0001, envelope zod schemas use no `.refine()` so the JSON Schema export stays faithful. Any cross-field invariants live in the server reducer, not the SDK schema.

## Acceptance Criteria

- [ ] `src/core/types.ts` exports envelope types only (`Event`, `Source`, `Subject`, `SubjectRef`, `Interaction`, `Session`, `Actor`, `PushEnvironment`, `TypeId`); no `Push`, `WorkSchema`, `EngineeringSchema`, `ProductSchema`, `CodeQualitySchema` types.
- [ ] `src/core/schemas.ts` exports envelope zod schemas only (`eventSchema`, `sourceSchema`, `subjectSchema`, `subjectRefSchema`, `interactionRequestSchema`, `interactionResponseSchema`); same exclusion.
- [ ] `Source` is required on `eventSchema`; `sourceSchema` enforces `name` (string) and `sdkVersion` (string) as required, `version` and `attributes` as optional.
- [ ] `parseTypeId(s)` returns `{ domain, entity, action, version }` or `null`; rejects malformed IDs against the regex `^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*){2}\.v[1-9]\d*$`.
- [ ] `formatTypeId(parts)` round-trips with `parseTypeId`.
- [ ] No envelope schema uses `.refine()`.
- [ ] Sub-path exports (`./types`, `./schemas`) still resolve; their contents shrink.
- [ ] All previously failing imports across the codebase are either deleted or migrated to envelope types.

## Tasks

### 1. Define Source (zod + types)

status: completed
commitSha: 2057f08

**Files to modify/create:**
- `src/core/source.ts` (new — Source schema + type + auto-fill helpers)
- `src/core/__tests__/source.test.ts` (new)

**Expected changes:**
- `sourceSchema` zod accepts `{ name, sdkVersion, version?, attributes? }` with `attributes` typed as `z.record(z.union([z.string(), z.number(), z.boolean()]))`.
- `name` and `sdkVersion` required; `version` and `attributes` optional.
- `mergeSource(auto, override)` helper merges caller-supplied source with auto-detected source (override fields win; empty-string overrides treated as gaps).
- `SDK_VERSION` resolved from package.json at module-load via `import.meta.url` + `readFileSync`. ESM-compatible; works in vitest (src/) and shipped npm tarball (dist/).
- Tests cover: shape validation, merge semantics (caller wins on conflict, gaps filled by auto, empty strings treated as gaps), key-presence vs explicit-undefined for optional fields, SDK_VERSION non-empty + semver pattern.

### 2. Define Event envelope (zod + types)

status: completed
commitSha: 97d695e

**Files to modify/create:**
- `src/core/event.ts` (new — envelope schema + Event<T> type)
- `src/core/subject.ts` (new — subjectRefSchema; Task 3 adds subjectSchema)
- `src/core/__tests__/event.test.ts` (new)

**Expected changes:**
- `eventSchema` zod accepts `{ type, data, source, subject?, occurredAt?, parentEventId?, traceId?, dedupeKey? }`. `source` is required (uses `sourceSchema` from Task 1). String fields validated with `.min(1)`.
- `data` is `z.unknown()` — opaque pass-through; per-type schema validates server-side.
- `Event<T>` generic type with `Readonly<...>` wrap for immutability discipline. Defaults to `unknown`.
- `subjectRefSchema` defined in `src/core/subject.ts` (Task 3 adds the promotion `subjectSchema` to the same file).
- Tests cover: shape validation, missing required fields, unknown top-level fields tolerated and stripped (forward-compat), no refinements present (asserted via `_def.typeName === 'ZodObject'`), reference-equality delegation to sourceSchema/subjectRefSchema.

### 3. Define Subject and SubjectRef

status: completed
commitSha: 04d4128

**Files to modify/create:**
- `src/core/subject.ts` (new)
- `src/core/subject.test.ts` (new)
- `src/core/types.ts`
- `src/core/schemas.ts`

**Expected changes:**
- `subjectRefSchema` for `{ type, id }` — used inside Event and Interaction envelopes.
- `subjectSchema` for promotion API — `{ type, externalId, displayName, metadata? }`.
- Tests cover both shapes.

### 4. Define Interaction envelope

status: completed
commitSha: 38dd600

**Files to modify/create:**
- `src/core/interaction.ts` (new)
- `src/core/interaction.test.ts` (new)
- `src/core/types.ts`
- `src/core/schemas.ts`

**Expected changes:**
- `interactionRequestSchema` for the request side of `runInteraction`: `{ id, input, subject? }`.
- `interactionResponseSchema` for the response: `{ events: { id, type }[] }` (final shape pinned to server's response — adjust during integration).
- Tests cover request/response round-trip.

### 5. Type-ID grammar (parse + format)

status: completed
commitSha: 28a23ee

**Files to modify/create:**
- `src/core/type-id.ts` (new)
- `src/core/type-id.test.ts` (new)

**Expected changes:**
- Regex: `^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*){2}\.v[1-9]\d*$`. Lowercase only, underscores allowed within segments, version is `v1`+ (no `v0`, no leading zeros).
- `parseTypeId('engineering.deploy.completed.v1')` → `{ domain: 'engineering', entity: 'deploy', action: 'completed', version: 1 }`.
- `formatTypeId(parts)` reverses.
- Tests: valid IDs (simple, with underscores like `engineering.health.status_changed.v1`, multi-digit versions like `v12`), invalid IDs (uppercase, leading-zero version `v01`, version `v0`, missing version, extra segments, special chars, empty segments).

### 6. Source construction helpers (Session / Actor / PushEnvironment / detectAgent)

status: completed
commitSha: 34498ed

**Files to modify/create:**
- `src/agent-detect.ts`
- `src/agent-detect.test.ts`
- `src/core/types.ts`
- `src/core/schemas.ts`

**Expected changes:**
- `Session`, `Actor`, `PushEnvironment` shed push-payload-specific fields and become helpers that *construct* `Source` (rather than sit alongside it as parallel envelope concepts).
- `detectAgent()` returns a fully-formed `Source`: `name: 'ci'` for known CI providers (GitHub Actions, GitLab, Circle, ...) with `attributes.provider`/`runId`/`workflow` populated; `name: 'sdk'` otherwise.
- `attributes.machine` populated only when `NO_TICKETS_INCLUDE_MACHINE=1`; value is a hashed hostname (per-installation salt stored at `~/.notickets/.machine-salt`), never the raw hostname.
- Tests update for the new shape; tests for hashed-machine path assert the salt file is created if missing and the hash is stable across runs with the same salt.

### 7. Remove push payload schemas

status: completed
commitSha: fb8cc8a

**Files to modify/create:**
- `src/core/types.ts`
- `src/core/schemas.ts`
- `src/core/__tests__/*` — delete tests for removed schemas
- Any existing `src/commands/push.ts` import sites (Feature 2 of this PRD will remove the command itself, but the schema removal must not leave dangling imports)

**Expected changes:**
- Delete: `Push`, `WorkSchema`, `WorkEntity`, `EngineeringSchema`, `EngineeringTask`, `EngineeringReview`, `ProductSchema`, `ProductUpdate`, `CodeQualitySchema`, and their zod equivalents.
- Resulting `types.ts` and `schemas.ts` contain only envelope/primitive shapes.
- Type-check passes after the delete; failing imports are addressed by Feature 2 removing the push command surface.

### 8. Sub-path export verification

status: completed
commitSha: 6b859b5

**Files to modify/create:**
- `package.json`
- New verification test under `src/__tests__/exports.test.ts` if missing

**Expected changes:**
- `./types` exports envelope types and nothing else.
- `./schemas` exports envelope zod and nothing else.
- Test verifies the export surface explicitly so future drift fails CI.

## Dependencies

- ADR-0001 (governs the boundary).
- Server-side `event-repository-foundation` Feature 1 — schemas the SDK must align with on the wire.

## Testing Strategy

### Unit Tests
- Envelope zod accepts well-formed envelopes; rejects malformed.
- Type-ID parser round-trips and rejects invalid forms.
- `detectAgent` outputs the trimmed shape across all CI providers and OS combinations covered today.

### Integration Tests
- Envelope schemas serialise to JSON Schema cleanly (no `.refine()` warnings).
- Export surface test fails when a domain payload is reintroduced.
