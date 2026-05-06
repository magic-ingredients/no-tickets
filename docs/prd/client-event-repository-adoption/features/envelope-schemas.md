---
id: envelope-schemas
prd_id: client-event-repository-adoption
number: 1
title: Envelope Schemas + SDK Surface Reset
status: not_started
created: 2026-04-27
updated: 2026-04-27
---

# Feature: Envelope Schemas + SDK Surface Reset

## Description

Replace the current push-payload-shaped exports in `src/core/types.ts` and `src/core/schemas.ts` with envelope-only types: `Event`, `Subject`, `Interaction`, `Session`, `Actor`, `SubjectRef`, plus the type-ID grammar parser. The SDK ships nothing about domain payloads — that's the server registry's job per ADR-0001.

This is the most disruptive feature in the PRD. Every existing consumer that imports `Push`, `WorkSchema`, `EngineeringSchema`, `ProductSchema`, `CodeQualitySchema`, etc. will fail to type-check after this lands. That's intentional: ADR-0001 and the no-v1-backcompat stance accept this as a clean cut. tiny-brain and any other internal consumer migrate as part of Feature 2.

### Wire-format types

```ts
type Event<T = unknown> = {
  readonly type: TypeId;
  readonly data: T;
  readonly subject?: SubjectRef;
  readonly source: string;
  readonly occurredAt?: string;
  readonly parentEventId?: string;
  readonly traceId?: string;
  readonly dedupeKey?: string;
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

type TypeId = `${string}.${string}.${string}.v${number}`;
```

`Session`, `Actor`, and `PushEnvironment` survive but lose their push-specific fields and become inputs to `emitEvent`'s `source` / actor inference instead.

### Refinement ban

Per ADR-0001, envelope zod schemas use no `.refine()` so the JSON Schema export stays faithful. Any cross-field invariants live in the server reducer, not the SDK schema.

## Acceptance Criteria

- [ ] `src/core/types.ts` exports envelope types only; no `Push`, `WorkSchema`, `EngineeringSchema`, `ProductSchema`, `CodeQualitySchema` types.
- [ ] `src/core/schemas.ts` exports envelope zod schemas only; same exclusion.
- [ ] `parseTypeId(s)` returns `{ domain, entity, action, version }` or `null`; rejects malformed IDs.
- [ ] `formatTypeId(parts)` round-trips with `parseTypeId`.
- [ ] No envelope schema uses `.refine()`.
- [ ] Sub-path exports (`./types`, `./schemas`) still resolve; their contents shrink.
- [ ] All previously failing imports across the codebase are either deleted or migrated to envelope types.

## Tasks

### 1. Define Event envelope (zod + types)

**Files to modify/create:**
- `src/core/event.ts` (new — envelope schema + type)
- `src/core/event.test.ts` (new)
- `src/core/types.ts`
- `src/core/schemas.ts`

**Expected changes:**
- `eventSchema` zod accepts `{ type, data, subject?, source, occurredAt?, parentEventId?, traceId?, dedupeKey? }`.
- `data` is `z.unknown()` — pass-through.
- `Event<T>` generic type for typed-payload narrowing in callers that opt into typed domain types later.
- Tests cover: shape validation, missing required fields, unknown fields rejected at top level (data is opaque), no refinements present.

### 2. Define Subject and SubjectRef

**Files to modify/create:**
- `src/core/subject.ts` (new)
- `src/core/subject.test.ts` (new)
- `src/core/types.ts`
- `src/core/schemas.ts`

**Expected changes:**
- `subjectRefSchema` for `{ type, id }` — used inside Event and Interaction envelopes.
- `subjectSchema` for promotion API — `{ type, externalId, displayName, metadata? }`.
- Tests cover both shapes.

### 3. Define Interaction envelope

**Files to modify/create:**
- `src/core/interaction.ts` (new)
- `src/core/interaction.test.ts` (new)
- `src/core/types.ts`
- `src/core/schemas.ts`

**Expected changes:**
- `interactionRequestSchema` for the request side of `runInteraction`: `{ id, input, subject? }`.
- `interactionResponseSchema` for the response: `{ events: { id, type }[] }` (final shape pinned to server's response — adjust during integration).
- Tests cover request/response round-trip.

### 4. Type-ID grammar (parse + format)

**Files to modify/create:**
- `src/core/type-id.ts` (new)
- `src/core/type-id.test.ts` (new)

**Expected changes:**
- `parseTypeId('engineering.deploy.completed.v1')` → `{ domain: 'engineering', entity: 'deploy', action: 'completed', version: 1 }`.
- `formatTypeId(parts)` reverses.
- Tests: malformed ids, missing version, multi-word actions, action with underscores (`status_changed`), version with extra digits (`v12`).

### 5. Trim Session / Actor / PushEnvironment to envelope shape

**Files to modify/create:**
- `src/agent-detect.ts`
- `src/agent-detect.test.ts`
- `src/core/types.ts`
- `src/core/schemas.ts`

**Expected changes:**
- `Session`, `Actor`, `PushEnvironment` shed any push-payload-specific fields.
- `detectAgent()` returns the trimmed shape; output feeds `emitEvent`'s `source` and the server's actor inference middleware.
- Tests update for the new shape; assertions about removed fields removed.

### 6. Remove push payload schemas

**Files to modify/create:**
- `src/core/types.ts`
- `src/core/schemas.ts`
- `src/core/__tests__/*` — delete tests for removed schemas
- Any existing `src/commands/push.ts` import sites (Feature 2 of this PRD will remove the command itself, but the schema removal must not leave dangling imports)

**Expected changes:**
- Delete: `Push`, `WorkSchema`, `WorkEntity`, `EngineeringSchema`, `EngineeringTask`, `EngineeringReview`, `ProductSchema`, `ProductUpdate`, `CodeQualitySchema`, and their zod equivalents.
- Resulting `types.ts` and `schemas.ts` contain only envelope/primitive shapes.
- Type-check passes after the delete; failing imports are addressed by Feature 2 removing the push command surface.

### 7. Sub-path export verification

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
