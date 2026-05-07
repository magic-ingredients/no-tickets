import { describe, it, expect } from 'vitest';
import { subjectRefSchema, subjectSchema, type Subject, type SubjectRef } from '../subject.js';

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

  it('rejects non-object root', () => {
    expect(() => subjectRefSchema.parse(null)).toThrow();
    expect(() => subjectRefSchema.parse('string')).toThrow();
    expect(() => subjectRefSchema.parse(42)).toThrow();
    expect(() => subjectRefSchema.parse([])).toThrow();
  });

  it('tolerates unknown top-level keys (forward-compat)', () => {
    const parsed = subjectRefSchema.parse({ type: 'feature', id: 'f-1', extra: true });
    expect(parsed).toEqual({ type: 'feature', id: 'f-1' });
    expect(Object.keys(parsed)).not.toContain('extra');
  });

  it('schema is not a ZodEffects (.refine() banned per ADR-0001)', () => {
    expect(subjectRefSchema._def.typeName).toBe('ZodObject');
  });

  it('SubjectRef enforces readonly fields at compile time', () => {
    const ref: SubjectRef = { type: 'feature', id: 'f-1' };
    // @ts-expect-error — readonly field
    ref.type = 'mut';
    // @ts-expect-error — readonly field
    ref.id = 'mut';
    expect(ref).toBeDefined();
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

  it('accepts empty metadata object', () => {
    const parsed = subjectSchema.parse({ ...minimalSubject, metadata: {} });
    expect(parsed.metadata).toEqual({});
  });

  it('rejects missing type', () => {
    const { type, ...rest } = minimalSubject;
    void type;
    expect(() => subjectSchema.parse(rest)).toThrow();
  });

  it('rejects missing externalId', () => {
    const { externalId, ...rest } = minimalSubject;
    void externalId;
    expect(() => subjectSchema.parse(rest)).toThrow();
  });

  it('rejects missing displayName', () => {
    const { displayName, ...rest } = minimalSubject;
    void displayName;
    expect(() => subjectSchema.parse(rest)).toThrow();
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

  it('rejects null metadata', () => {
    expect(() => subjectSchema.parse({ ...minimalSubject, metadata: null })).toThrow();
  });

  it('rejects non-object root', () => {
    expect(() => subjectSchema.parse(null)).toThrow();
    expect(() => subjectSchema.parse('string')).toThrow();
    expect(() => subjectSchema.parse(42)).toThrow();
    expect(() => subjectSchema.parse([])).toThrow();
  });

  it('tolerates unknown top-level keys (forward-compat)', () => {
    const parsed = subjectSchema.parse({ ...minimalSubject, futureField: 'allowed' });
    expect(parsed).toEqual(minimalSubject);
  });

  it('schema is not a ZodEffects (.refine() banned per ADR-0001)', () => {
    expect(subjectSchema._def.typeName).toBe('ZodObject');
  });

  it('Subject enforces readonly top-level fields at compile time', () => {
    const subject: Subject = minimalSubject;
    // @ts-expect-error — readonly
    subject.type = 'mut';
    // @ts-expect-error — readonly
    subject.externalId = 'mut';
    // @ts-expect-error — readonly
    subject.displayName = 'mut';
    expect(subject).toBeDefined();
  });

  it('Subject.metadata is deep-readonly at compile time', () => {
    const subject: Subject = { ...minimalSubject, metadata: { region: 'us-east-1' } };
    // @ts-expect-error — readonly index signature
    subject.metadata!.region = 'mut';
    expect(subject).toBeDefined();
  });
});
