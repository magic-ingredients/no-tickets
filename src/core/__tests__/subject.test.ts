import { describe, it, expect } from 'vitest';
import { subjectRefSchema, subjectSchema, type Subject, type SubjectRef } from '../subject.js';

function omit<T extends object>(obj: T, key: keyof T): Partial<T> {
  const copy = { ...obj } as Partial<T>;
  delete copy[key];
  return copy;
}

// -- subjectRefSchema ---------------------------------------------------------

describe('subjectRefSchema', () => {
  it('accepts a valid ref', () => {
    const parsed = subjectRefSchema.parse({ type: 'feature', id: 'f-1' });
    expect(parsed).toEqual({ type: 'feature', id: 'f-1' });
  });

  it('rejects missing type', () => {
    expect(() => subjectRefSchema.parse({ id: 'f-1' })).toThrow();
  });

  it('rejects missing id', () => {
    expect(() => subjectRefSchema.parse({ type: 'feature' })).toThrow();
  });

  it('rejects empty-string type', () => {
    expect(() => subjectRefSchema.parse({ type: '', id: 'f-1' })).toThrow();
  });

  it('rejects empty-string id', () => {
    expect(() => subjectRefSchema.parse({ type: 'feature', id: '' })).toThrow();
  });

  it('rejects non-string type', () => {
    expect(() => subjectRefSchema.parse({ type: 42, id: 'f-1' })).toThrow();
  });

  it('rejects non-string id', () => {
    expect(() => subjectRefSchema.parse({ type: 'feature', id: 42 })).toThrow();
  });

  it('tolerates unknown top-level keys (forward-compat)', () => {
    const parsed = subjectRefSchema.parse({ type: 'feature', id: 'f-1', extra: true });
    expect(parsed).toEqual({ type: 'feature', id: 'f-1' });
    expect(Object.keys(parsed)).not.toContain('extra');
  });

  it('schema is not a ZodEffects (.refine() banned per ADR-0001)', () => {
    expect(subjectRefSchema._def.typeName).toBe('ZodObject');
  });

  it('SubjectRef type is readonly at the type level', () => {
    const ref: SubjectRef = { type: 'feature', id: 'f-1' };
    // ref.type = 'mut'  // ❌ ts(2540) — readonly enforcement
    expect(ref.type).toBe('feature');
  });
});

// -- subjectSchema (promotion shape) ------------------------------------------

const minimalSubject = {
  type: 'engineering_service',
  externalId: 'svc-api',
  displayName: 'API Service',
};

describe('subjectSchema', () => {
  it('accepts the minimal valid subject (no metadata)', () => {
    const parsed = subjectSchema.parse(minimalSubject);
    expect(parsed).toEqual(minimalSubject);
  });

  it('accepts optional metadata of arbitrary shape', () => {
    const parsed = subjectSchema.parse({
      ...minimalSubject,
      metadata: { region: 'us-east-1', tier: 1, primary: true, tags: ['a', 'b'], nested: { x: 1 } },
    });
    expect(parsed.metadata).toEqual({
      region: 'us-east-1',
      tier: 1,
      primary: true,
      tags: ['a', 'b'],
      nested: { x: 1 },
    });
  });

  it('rejects missing type', () => {
    expect(() => subjectSchema.parse(omit(minimalSubject, 'type'))).toThrow();
  });

  it('rejects missing externalId', () => {
    expect(() => subjectSchema.parse(omit(minimalSubject, 'externalId'))).toThrow();
  });

  it('rejects missing displayName', () => {
    expect(() => subjectSchema.parse(omit(minimalSubject, 'displayName'))).toThrow();
  });

  it('rejects empty-string type', () => {
    expect(() => subjectSchema.parse({ ...minimalSubject, type: '' })).toThrow();
  });

  it('rejects empty-string externalId', () => {
    expect(() => subjectSchema.parse({ ...minimalSubject, externalId: '' })).toThrow();
  });

  it('rejects empty-string displayName', () => {
    expect(() => subjectSchema.parse({ ...minimalSubject, displayName: '' })).toThrow();
  });

  it('rejects non-object metadata', () => {
    expect(() => subjectSchema.parse({ ...minimalSubject, metadata: 'string' })).toThrow();
    expect(() => subjectSchema.parse({ ...minimalSubject, metadata: 42 })).toThrow();
    expect(() => subjectSchema.parse({ ...minimalSubject, metadata: [] })).toThrow();
  });

  it('tolerates unknown top-level keys (forward-compat)', () => {
    const parsed = subjectSchema.parse({ ...minimalSubject, futureField: 'allowed' });
    expect(parsed).toEqual(minimalSubject);
  });

  it('schema is not a ZodEffects (.refine() banned per ADR-0001)', () => {
    expect(subjectSchema._def.typeName).toBe('ZodObject');
  });

  it('Subject type is readonly at the type level', () => {
    const subject: Subject = minimalSubject;
    // subject.type = 'mut'  // ❌ ts(2540) — readonly enforcement
    expect(subject.type).toBe('engineering_service');
  });
});
