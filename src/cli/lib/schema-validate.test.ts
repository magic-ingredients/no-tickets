import { describe, it, expect } from 'vitest';
import { validateEventLocally } from './schema-validate.js';

// Phase-1 contract for validateEventLocally:
// - Looks up the schema by type id from @magic-ingredients/no-tickets-schemas (byTypeId)
// - Returns [] when valid
// - Returns ValidationIssue[] when invalid (path joined with '.', message from Zod)
// - Returns { unknownType: true } when the type id is not in the registry
//
// Used by CLI publish (single + batch) to reject malformed payloads
// before the HTTP round-trip. Server is still authoritative.

describe('validateEventLocally', () => {
  it('returns an empty issues array for a known type with valid data', () => {
    const result = validateEventLocally('product.epic.created.v1', {
      epicId: 'epic-1',
      projectId: 'proj-1',
      title: 'My epic',
    });
    expect(result).toEqual([]);
  });

  it('returns ValidationIssue[] when a required field is missing', () => {
    const result = validateEventLocally('product.epic.created.v1', {
      epicId: 'epic-1',
      // projectId missing
      title: 'My epic',
    });
    expect(Array.isArray(result)).toBe(true);
    if (!Array.isArray(result)) return; // type-narrow for TS
    expect(result.length).toBeGreaterThan(0);
    const projectIdIssue = result.find((i) => i.path === 'projectId');
    expect(projectIdIssue).toBeDefined();
    expect(typeof projectIdIssue?.message).toBe('string');
  });

  it('joins nested paths with "." (e.g. agent.id) when a nested field fails', () => {
    // ai.completion.recorded.v1 has a nested `agent: { id, version }` object.
    // Empty string at agent.id fails .min(1); the issue path should be ['agent','id']
    // → joined to 'agent.id'. This is the test the previous implementation lacked.
    const result = validateEventLocally('ai.completion.recorded.v1', {
      callId: 'c1',
      sessionId: 's1',
      agent: { id: '', version: '1.0.0' }, // bad nested field
      provider: 'anthropic',
      model: 'm',
      modelVersion: null,
      inputTokens: 0,
      outputTokens: 0,
      durationMs: 0,
      stopReason: 'stop',
      toolCallCount: 0,
      contextUsed: null,
      systemPromptHash: 'a'.repeat(64),
      toolRegistryHash: 'a'.repeat(64),
    });
    expect(Array.isArray(result)).toBe(true);
    if (!Array.isArray(result)) return;
    const nested = result.find((i) => i.path === 'agent.id');
    expect(nested).toBeDefined();
    expect(nested?.path).toContain('.');
  });

  it('returns { unknownType: true } when the type id is not in the registry', () => {
    const result = validateEventLocally('definitely.not.registered.v9', { foo: 'bar' });
    expect(result).toEqual({ unknownType: true });
  });

  it('returns { unknownType: true } for inherited Object.prototype names (no prototype-chain leak)', () => {
    // Regression: `typeId in byTypeId` walked the prototype chain, so
    // 'toString' / 'hasOwnProperty' / 'valueOf' would slip past the guard
    // and crash on .safeParse(undefined). Object.hasOwn fixes it.
    for (const proto of ['toString', 'hasOwnProperty', 'valueOf', 'constructor']) {
      expect(validateEventLocally(proto, {})).toEqual({ unknownType: true });
    }
  });

  it('rejects extra keys with an unrecognized_keys issue when the schema is .strict()', () => {
    const result = validateEventLocally('product.epic.created.v1', {
      epicId: 'epic-1',
      projectId: 'proj-1',
      title: 'My epic',
      extraneous: 'should-be-rejected',
    });
    expect(Array.isArray(result)).toBe(true);
    if (!Array.isArray(result)) return;
    // Find an issue whose message references the extraneous key — pinning that
    // strict-mode rejection actually fires, not just any old issue.
    const strictIssue = result.find((i) => i.message.includes('extraneous'));
    expect(strictIssue).toBeDefined();
  });

  it('returns ValidationIssue[] (not throw) when data is not an object', () => {
    // Trust-boundary guard: callers pass `unknown`. null / array / primitive
    // must be rejected by the schema, not crash the validator.
    for (const bad of [null, 'a string', 42, true, ['array']]) {
      const result = validateEventLocally('product.epic.created.v1', bad);
      expect(Array.isArray(result)).toBe(true);
      if (!Array.isArray(result)) continue;
      expect(result.length).toBeGreaterThan(0);
    }
  });
});
