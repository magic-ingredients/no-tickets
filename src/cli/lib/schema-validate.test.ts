import { describe, it, expect } from 'vitest';
import { isKnownEventType, validateEventLocally } from './schema-validate.js';

// Contract:
// - isKnownEventType(typeId) is a type guard: true for ids in the bundled
//   byTypeId map, false otherwise. Uses Object.hasOwn so prototype names
//   ('toString', 'hasOwnProperty', etc.) don't slip past.
// - validateEventLocally(typeId, data) takes an EventTypeId (already
//   narrowed via isKnownEventType) and returns:
//     []                       on success
//     ValidationIssue[]        on schema failure (path joined with '.', message from Zod)
// - The split makes "unknown event type" a structural impossibility at the
//   validateEventLocally seam — no union return, no defensive branches.

describe('isKnownEventType', () => {
  it('returns true for a registered event type id', () => {
    expect(isKnownEventType('product.epic.created.v1')).toBe(true);
  });

  it('returns false for an unregistered event type id', () => {
    expect(isKnownEventType('definitely.not.registered.v9')).toBe(false);
  });

  it('returns false for inherited Object.prototype names (no prototype-chain leak)', () => {
    // Regression: a `typeId in byTypeId` check walks the prototype chain,
    // so 'toString' / 'hasOwnProperty' / 'valueOf' / 'constructor' would
    // slip past the guard and crash a downstream byTypeId[typeId].safeParse
    // call. Object.hasOwn under the hood fixes this.
    for (const proto of ['toString', 'hasOwnProperty', 'valueOf', 'constructor']) {
      expect(isKnownEventType(proto)).toBe(false);
    }
  });

  it('returns false for the empty string', () => {
    expect(isKnownEventType('')).toBe(false);
  });
});

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
    expect(result.length).toBeGreaterThan(0);
    const projectIdIssue = result.find((i) => i.path === 'projectId');
    expect(projectIdIssue).toBeDefined();
    expect(typeof projectIdIssue?.message).toBe('string');
  });

  it('joins nested paths with "." (e.g. agent.id) when a nested field fails', () => {
    // ai.completion.recorded.v1 has a nested `agent: { id, version }` object.
    // Empty string at agent.id fails .min(1); the issue path is ['agent','id']
    // → joined to 'agent.id'. Pins the join separator — joining with '' or
    // '/' would produce a different string and fail this assertion.
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
    const nested = result.find((i) => i.path === 'agent.id');
    expect(nested).toBeDefined();
    expect(nested?.path).toBe('agent.id');
    // Negative pin against alternate separators that would silently pass a
    // join('/') or join('') regression
    expect(nested?.path).not.toBe('agentid');
    expect(nested?.path).not.toBe('agent/id');
  });

  it('rejects extra keys with an unrecognized_keys issue when the schema is .strict()', () => {
    const result = validateEventLocally('product.epic.created.v1', {
      epicId: 'epic-1',
      projectId: 'proj-1',
      title: 'My epic',
      extraneous: 'should-be-rejected',
    });
    // Pin that the strict-mode rejection actually fires AND names the key —
    // dropping .strict() from the schema or replacing it with .passthrough()
    // would silently slip past a length-only assertion.
    const strictIssue = result.find((i) => i.message.includes('extraneous'));
    expect(strictIssue).toBeDefined();
  });

  it('returns ValidationIssue[] (not throw) when data is not an object', () => {
    // Trust-boundary guard: callers pass `unknown`. null / array / primitive
    // must be rejected by the schema, not crash the validator.
    for (const bad of [null, 'a string', 42, true, ['array']]) {
      const result = validateEventLocally('product.epic.created.v1', bad);
      expect(result.length).toBeGreaterThan(0);
    }
  });
});
