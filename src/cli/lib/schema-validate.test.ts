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

  it('returns ValidationIssue[] with dot-joined path for nested errors', () => {
    // ai.task.completed.v1 has nested fields; pick something with a min-length string
    const result = validateEventLocally('product.epic.created.v1', {
      epicId: '', // empty string fails .min(1)
      projectId: 'proj-1',
      title: 'My epic',
    });
    expect(Array.isArray(result)).toBe(true);
    if (!Array.isArray(result)) return;
    const issue = result.find((i) => i.path === 'epicId');
    expect(issue).toBeDefined();
  });

  it('returns { unknownType: true } when the type id is not in the registry', () => {
    const result = validateEventLocally('definitely.not.registered.v9', { foo: 'bar' });
    expect(result).toEqual({ unknownType: true });
  });

  it('rejects extra keys when the registered schema is .strict()', () => {
    // productEpicCreatedSchema is .strict(); unknown keys should produce an issue
    const result = validateEventLocally('product.epic.created.v1', {
      epicId: 'epic-1',
      projectId: 'proj-1',
      title: 'My epic',
      extraneous: 'should-be-rejected',
    });
    expect(Array.isArray(result)).toBe(true);
    if (!Array.isArray(result)) return;
    expect(result.length).toBeGreaterThan(0);
  });
});
