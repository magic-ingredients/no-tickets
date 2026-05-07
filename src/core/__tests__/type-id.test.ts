import { describe, it, expect } from 'vitest';
import { parseTypeId, formatTypeId, TYPE_ID_REGEX, type TypeIdParts } from '../type-id.js';

describe('parseTypeId', () => {
  it('parses a basic type id', () => {
    expect(parseTypeId('engineering.deploy.completed.v1')).toEqual({
      domain: 'engineering',
      entity: 'deploy',
      action: 'completed',
      version: 1,
    });
  });

  it('parses an id with underscores in segments', () => {
    expect(parseTypeId('engineering.health.status_changed.v1')).toEqual({
      domain: 'engineering',
      entity: 'health',
      action: 'status_changed',
      version: 1,
    });
  });

  it('parses an id with digits in segments', () => {
    expect(parseTypeId('product.feature.created.v1')).toEqual({
      domain: 'product',
      entity: 'feature',
      action: 'created',
      version: 1,
    });
  });

  it('parses multi-digit versions', () => {
    expect(parseTypeId('domain.entity.action.v12')?.version).toBe(12);
    expect(parseTypeId('domain.entity.action.v100')?.version).toBe(100);
  });

  it('parses segments containing digits after the leading letter', () => {
    expect(parseTypeId('a1.b2.c3.v1')).toEqual({
      domain: 'a1',
      entity: 'b2',
      action: 'c3',
      version: 1,
    });
  });

  it('returns null for malformed: missing version', () => {
    expect(parseTypeId('engineering.deploy.completed')).toBeNull();
  });

  it('returns null for malformed: empty input', () => {
    expect(parseTypeId('')).toBeNull();
  });

  it('returns null for malformed: extra segment', () => {
    expect(parseTypeId('a.b.c.d.v1')).toBeNull();
  });

  it('returns null for malformed: too few segments', () => {
    expect(parseTypeId('a.b.v1')).toBeNull();
    expect(parseTypeId('a.v1')).toBeNull();
  });

  it('returns null for uppercase letters', () => {
    expect(parseTypeId('Engineering.deploy.completed.v1')).toBeNull();
    expect(parseTypeId('engineering.Deploy.completed.v1')).toBeNull();
    expect(parseTypeId('engineering.deploy.Completed.v1')).toBeNull();
    expect(parseTypeId('engineering.deploy.completed.V1')).toBeNull();
  });

  it('returns null for leading-digit segments', () => {
    expect(parseTypeId('1eng.deploy.completed.v1')).toBeNull();
    expect(parseTypeId('eng.1deploy.completed.v1')).toBeNull();
    expect(parseTypeId('eng.deploy.1completed.v1')).toBeNull();
  });

  it('returns null for leading-underscore segments', () => {
    expect(parseTypeId('_eng.deploy.completed.v1')).toBeNull();
  });

  it('returns null for empty segments', () => {
    expect(parseTypeId('.deploy.completed.v1')).toBeNull();
    expect(parseTypeId('eng..completed.v1')).toBeNull();
    expect(parseTypeId('eng.deploy..v1')).toBeNull();
    expect(parseTypeId('eng.deploy.completed.')).toBeNull();
  });

  it('returns null for v0 (versions start at v1)', () => {
    expect(parseTypeId('eng.deploy.completed.v0')).toBeNull();
  });

  it('returns null for leading-zero versions', () => {
    expect(parseTypeId('eng.deploy.completed.v01')).toBeNull();
    expect(parseTypeId('eng.deploy.completed.v002')).toBeNull();
  });

  it('returns null for non-numeric versions', () => {
    expect(parseTypeId('eng.deploy.completed.va')).toBeNull();
    expect(parseTypeId('eng.deploy.completed.v1a')).toBeNull();
  });

  it('returns null for missing v prefix on version', () => {
    expect(parseTypeId('eng.deploy.completed.1')).toBeNull();
  });

  it('returns null for special chars in segments', () => {
    expect(parseTypeId('eng-domain.deploy.completed.v1')).toBeNull();
    expect(parseTypeId('eng/domain.deploy.completed.v1')).toBeNull();
    expect(parseTypeId('eng domain.deploy.completed.v1')).toBeNull();
  });

  it('returns null for non-string input', () => {
    // @ts-expect-error — runtime safety; callers may pass unknown
    expect(parseTypeId(null)).toBeNull();
    // @ts-expect-error
    expect(parseTypeId(undefined)).toBeNull();
    // @ts-expect-error
    expect(parseTypeId(123)).toBeNull();
  });
});

describe('formatTypeId', () => {
  it('formats a basic parts object', () => {
    expect(
      formatTypeId({
        domain: 'engineering',
        entity: 'deploy',
        action: 'completed',
        version: 1,
      }),
    ).toBe('engineering.deploy.completed.v1');
  });

  it('formats segments with underscores', () => {
    expect(
      formatTypeId({
        domain: 'engineering',
        entity: 'health',
        action: 'status_changed',
        version: 1,
      }),
    ).toBe('engineering.health.status_changed.v1');
  });

  it('formats multi-digit versions', () => {
    expect(
      formatTypeId({ domain: 'a', entity: 'b', action: 'c', version: 12 }),
    ).toBe('a.b.c.v12');
  });
});

describe('round-trip (parse → format → parse)', () => {
  const cases = [
    'engineering.deploy.completed.v1',
    'engineering.health.status_changed.v1',
    'product.feature.created.v2',
    'ai.completion.recorded.v100',
    'a1.b2.c3.v1',
  ];

  for (const id of cases) {
    it(`round-trips ${id}`, () => {
      const parts = parseTypeId(id);
      expect(parts).not.toBeNull();
      const formatted = formatTypeId(parts as TypeIdParts);
      expect(formatted).toBe(id);
      expect(parseTypeId(formatted)).toEqual(parts);
    });
  }
});

describe('TYPE_ID_REGEX', () => {
  it('exports the regex used for validation', () => {
    expect(TYPE_ID_REGEX).toBeInstanceOf(RegExp);
    expect(TYPE_ID_REGEX.test('engineering.deploy.completed.v1')).toBe(true);
    expect(TYPE_ID_REGEX.test('Engineering.deploy.completed.v1')).toBe(false);
  });
});
