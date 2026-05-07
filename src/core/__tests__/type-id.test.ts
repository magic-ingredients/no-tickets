import { describe, it, expect } from 'vitest';
import {
  parseTypeId,
  formatTypeId,
  isTypeId,
  TYPE_ID_REGEX,
  type TypeIdParts,
} from '../type-id.js';

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

  it('returns null for empty input', () => {
    expect(parseTypeId('')).toBeNull();
  });

  it('returns null for extra segment', () => {
    expect(parseTypeId('a.b.c.d.v1')).toBeNull();
  });

  it('returns null for too few segments', () => {
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

  it('returns null for whitespace-padded input', () => {
    expect(parseTypeId(' eng.deploy.completed.v1')).toBeNull();
    expect(parseTypeId('eng.deploy.completed.v1 ')).toBeNull();
    expect(parseTypeId('\teng.deploy.completed.v1')).toBeNull();
    expect(parseTypeId('eng.deploy.completed.v1\n')).toBeNull();
  });

  it('returns null for unsafe-integer versions (would lose precision)', () => {
    // 2^53 = 9007199254740992 is the largest safe integer; one more is unsafe.
    expect(parseTypeId('a.b.c.v9007199254740993')).toBeNull();
    expect(parseTypeId('a.b.c.v99999999999999999')).toBeNull();
  });

  it('accepts version at the safe-integer boundary', () => {
    const parts = parseTypeId('a.b.c.v9007199254740991');
    expect(parts?.version).toBe(9007199254740991);
  });

  it('returns null for non-string input', () => {
    expect(parseTypeId(null)).toBeNull();
    expect(parseTypeId(undefined)).toBeNull();
    expect(parseTypeId(123)).toBeNull();
    expect(parseTypeId({})).toBeNull();
    expect(parseTypeId([])).toBeNull();
    expect(parseTypeId(true)).toBeNull();
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

  it('throws on uppercase domain', () => {
    expect(() =>
      formatTypeId({ domain: 'Eng', entity: 'b', action: 'c', version: 1 }),
    ).toThrow();
  });

  it('throws on empty-string segment', () => {
    expect(() =>
      formatTypeId({ domain: '', entity: 'b', action: 'c', version: 1 }),
    ).toThrow();
    expect(() =>
      formatTypeId({ domain: 'a', entity: '', action: 'c', version: 1 }),
    ).toThrow();
    expect(() =>
      formatTypeId({ domain: 'a', entity: 'b', action: '', version: 1 }),
    ).toThrow();
  });

  it('throws on negative version', () => {
    expect(() =>
      formatTypeId({ domain: 'a', entity: 'b', action: 'c', version: -1 }),
    ).toThrow();
  });

  it('throws on zero version', () => {
    expect(() =>
      formatTypeId({ domain: 'a', entity: 'b', action: 'c', version: 0 }),
    ).toThrow();
  });

  it('throws on non-integer version', () => {
    expect(() =>
      formatTypeId({ domain: 'a', entity: 'b', action: 'c', version: 1.5 }),
    ).toThrow();
  });

  it('throws on segment with embedded dot', () => {
    expect(() =>
      formatTypeId({ domain: 'a.b', entity: 'c', action: 'd', version: 1 }),
    ).toThrow();
  });
});

describe('round-trip stability', () => {
  const cases: TypeIdParts[] = [
    { domain: 'engineering', entity: 'deploy', action: 'completed', version: 1 },
    { domain: 'engineering', entity: 'health', action: 'status_changed', version: 1 },
    { domain: 'product', entity: 'feature', action: 'created', version: 2 },
    { domain: 'ai', entity: 'completion', action: 'recorded', version: 100 },
    { domain: 'a1', entity: 'b2', action: 'c3', version: 1 },
  ];

  for (const parts of cases) {
    const id = formatTypeId(parts);
    it(`construct → format → parse: ${id}`, () => {
      expect(parseTypeId(id)).toEqual(parts);
    });

    it(`parse → format → parse: ${id}`, () => {
      const parsed = parseTypeId(id);
      expect(parsed).not.toBeNull();
      if (parsed === null) return;
      expect(formatTypeId(parsed)).toBe(id);
    });
  }
});

describe('isTypeId', () => {
  it('returns true for a valid type id', () => {
    expect(isTypeId('engineering.deploy.completed.v1')).toBe(true);
  });

  it('returns false for invalid strings', () => {
    expect(isTypeId('Engineering.deploy.completed.v1')).toBe(false);
    expect(isTypeId('eng.deploy.completed.v0')).toBe(false);
    expect(isTypeId('not.enough')).toBe(false);
    expect(isTypeId('')).toBe(false);
  });

  it('returns false for non-strings', () => {
    expect(isTypeId(null)).toBe(false);
    expect(isTypeId(undefined)).toBe(false);
    expect(isTypeId(123)).toBe(false);
    expect(isTypeId({})).toBe(false);
  });

  it('narrows type at the call site', () => {
    const value: unknown = 'engineering.deploy.completed.v1';
    if (isTypeId(value)) {
      // value is now narrowed to string
      const narrowed: string = value;
      expect(narrowed.length).toBeGreaterThan(0);
    }
  });
});

describe('TYPE_ID_REGEX', () => {
  it('exports the regex used for validation', () => {
    expect(TYPE_ID_REGEX).toBeInstanceOf(RegExp);
  });
});
