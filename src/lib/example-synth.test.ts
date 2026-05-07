import { describe, it, expect } from 'vitest';
import { synthesiseExample } from './example-synth.js';

describe('synthesiseExample — primitives', () => {
  it('produces an empty string for type: string', () => {
    expect(synthesiseExample({ type: 'string' })).toBe('');
  });

  it('produces 0 for type: number', () => {
    expect(synthesiseExample({ type: 'number' })).toBe(0);
  });

  it('produces 0 for type: integer', () => {
    expect(synthesiseExample({ type: 'integer' })).toBe(0);
  });

  it('produces false for type: boolean', () => {
    expect(synthesiseExample({ type: 'boolean' })).toBe(false);
  });

  it('produces null for type: null', () => {
    expect(synthesiseExample({ type: 'null' })).toBeNull();
  });
});

describe('synthesiseExample — defaults beat type placeholders', () => {
  it('uses default when present (string)', () => {
    expect(synthesiseExample({ type: 'string', default: 'hello' })).toBe('hello');
  });

  it('uses default when present (number)', () => {
    expect(synthesiseExample({ type: 'number', default: 42 })).toBe(42);
  });

  it('uses default of false even when type would default to false (no truthy bias)', () => {
    expect(synthesiseExample({ type: 'boolean', default: false })).toBe(false);
  });
});

describe('synthesiseExample — enums', () => {
  it('uses the first enum value when no default is present', () => {
    expect(synthesiseExample({ type: 'string', enum: ['a', 'b', 'c'] })).toBe('a');
  });

  it('default beats enum first value', () => {
    expect(synthesiseExample({ type: 'string', enum: ['a', 'b'], default: 'b' })).toBe('b');
  });

  it('enum without a type still works', () => {
    expect(synthesiseExample({ enum: ['x', 'y'] })).toBe('x');
  });
});

describe('synthesiseExample — objects', () => {
  it('produces an empty object for type: object with no properties', () => {
    expect(synthesiseExample({ type: 'object' })).toEqual({});
  });

  it('synthesises every property of an object', () => {
    expect(
      synthesiseExample({
        type: 'object',
        properties: {
          name: { type: 'string' },
          age: { type: 'integer' },
        },
      }),
    ).toEqual({ name: '', age: 0 });
  });

  it('respects per-property defaults', () => {
    expect(
      synthesiseExample({
        type: 'object',
        properties: {
          name: { type: 'string', default: 'Ada' },
          plan: { type: 'string', enum: ['free', 'pro'] },
        },
      }),
    ).toEqual({ name: 'Ada', plan: 'free' });
  });

  it('recurses into nested objects', () => {
    expect(
      synthesiseExample({
        type: 'object',
        properties: {
          user: {
            type: 'object',
            properties: {
              email: { type: 'string', default: 'a@b.c' },
            },
          },
        },
      }),
    ).toEqual({ user: { email: 'a@b.c' } });
  });
});

describe('synthesiseExample — arrays', () => {
  it('produces a single-item array from items', () => {
    expect(
      synthesiseExample({
        type: 'array',
        items: { type: 'string' },
      }),
    ).toEqual(['']);
  });

  it('produces an empty array when items is missing', () => {
    expect(synthesiseExample({ type: 'array' })).toEqual([]);
  });
});

describe('synthesiseExample — fallbacks', () => {
  it('produces null for a wholly unknown shape', () => {
    expect(synthesiseExample({})).toBeNull();
  });

  it('produces null for an unrecognised type', () => {
    expect(synthesiseExample({ type: 'lambda-soup' as unknown as 'string' })).toBeNull();
  });

  it('produces null at the trust boundary for a primitive input', () => {
    expect(synthesiseExample('not-a-schema')).toBeNull();
    expect(synthesiseExample(42)).toBeNull();
    expect(synthesiseExample(true)).toBeNull();
  });

  it('produces null at the trust boundary for null', () => {
    expect(synthesiseExample(null)).toBeNull();
  });

  it('produces null at the trust boundary for array input', () => {
    expect(synthesiseExample([{ type: 'string' }])).toBeNull();
  });

  it('falls through to type placeholder when enum is an empty array', () => {
    expect(synthesiseExample({ type: 'string', enum: [] })).toBe('');
    expect(synthesiseExample({ type: 'integer', enum: [] })).toBe(0);
  });
});
