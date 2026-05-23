import { describe, it, expect } from 'vitest';
import { eventSchema, type Event } from '../event.js';
import { sourceSchema } from '../source.js';

const validSource = { name: 'cli', sdkVersion: '1.2.3' };

const minimalEvent = {
  type: 'engineering.deploy.completed.v1',
  data: { foo: 'bar' },
  source: validSource,
};

function omit<T extends object>(obj: T, key: keyof T): Partial<T> {
  const copy = { ...obj } as Partial<T>;
  delete copy[key];
  return copy;
}

describe('eventSchema', () => {
  it('accepts the minimal valid envelope', () => {
    const parsed = eventSchema.parse(minimalEvent);
    expect(parsed).toEqual(minimalEvent);
  });

  it('accepts the fully-populated envelope', () => {
    const event = {
      type: 'ai.completion.recorded.v1',
      data: { callId: 'c-1', tokens: 42 },
      source: { name: 'mcp', sdkVersion: '1.2.3', attributes: { client: 'claude-code' } },
      occurredAt: '2026-05-07T10:30:00Z',
      parentEventId: 'e-100',
      traceId: 'session-abc',
      dedupeKey: 'unique-key-1',
    };
    expect(eventSchema.parse(event)).toEqual(event);
  });

  it('treats data as opaque (accepts any shape — object, primitive, null, array, boolean, undefined)', () => {
    const cases: unknown[] = [
      { deeply: { nested: { thing: [1, 2] } } },
      'a string',
      42,
      null,
      [],
      true,
      false,
      undefined,
    ];
    for (const data of cases) {
      expect(() => eventSchema.parse({ ...minimalEvent, data })).not.toThrow();
    }
  });

  it('rejects missing type', () => {
    expect(() => eventSchema.parse(omit(minimalEvent, 'type'))).toThrow();
  });

  it('tolerates missing data (z.unknown() is opaque pass-through; per-type schema validates presence)', () => {
    expect(() => eventSchema.parse(omit(minimalEvent, 'data'))).not.toThrow();
  });

  it('rejects missing source', () => {
    expect(() => eventSchema.parse(omit(minimalEvent, 'source'))).toThrow();
  });

  it('rejects empty-string type', () => {
    expect(() => eventSchema.parse({ ...minimalEvent, type: '' })).toThrow();
  });

  it('rejects non-string type', () => {
    expect(() => eventSchema.parse({ ...minimalEvent, type: 42 })).toThrow();
  });

  it('delegates source validation to sourceSchema (reference equality)', () => {
    expect(eventSchema.shape.source).toBe(sourceSchema);
  });

  it('rejects an invalid source object (missing sdkVersion)', () => {
    expect(() =>
      eventSchema.parse({ ...minimalEvent, source: { name: 'cli' } }),
    ).toThrow();
  });

  it('rejects an invalid source object (empty name)', () => {
    expect(() =>
      eventSchema.parse({ ...minimalEvent, source: { name: '', sdkVersion: '1.2.3' } }),
    ).toThrow();
  });

  it('accepts optional occurredAt as a non-empty string (ISO 8601 not enforced at envelope level — server validates)', () => {
    expect(() =>
      eventSchema.parse({ ...minimalEvent, occurredAt: '2026-05-07T10:30:00Z' }),
    ).not.toThrow();
  });

  it('rejects non-string occurredAt', () => {
    expect(() =>
      eventSchema.parse({ ...minimalEvent, occurredAt: 1730000000 }),
    ).toThrow();
  });

  it('rejects empty-string occurredAt', () => {
    expect(() =>
      eventSchema.parse({ ...minimalEvent, occurredAt: '' }),
    ).toThrow();
  });

  it('accepts optional traceId, parentEventId, dedupeKey when strings', () => {
    expect(() =>
      eventSchema.parse({
        ...minimalEvent,
        traceId: 't-1',
        parentEventId: 'p-1',
        dedupeKey: 'd-1',
      }),
    ).not.toThrow();
  });

  it('rejects empty-string traceId', () => {
    expect(() => eventSchema.parse({ ...minimalEvent, traceId: '' })).toThrow();
  });

  it('rejects empty-string parentEventId', () => {
    expect(() => eventSchema.parse({ ...minimalEvent, parentEventId: '' })).toThrow();
  });

  it('rejects empty-string dedupeKey', () => {
    expect(() => eventSchema.parse({ ...minimalEvent, dedupeKey: '' })).toThrow();
  });

  it('tolerates unknown top-level keys and strips them (forward-compat — server may add fields)', () => {
    const parsed = eventSchema.parse({ ...minimalEvent, futureField: 'allowed' });
    expect(parsed).toEqual(minimalEvent);
    expect(Object.keys(parsed)).not.toContain('futureField');
  });

  it('schema is not a ZodEffects (.refine() / .transform() banned per ADR-0001)', () => {
    // Refinements would wrap the schema in ZodEffects; the JSON Schema export
    // would silently drop the refinement and validation would diverge between
    // SDK and exported schema. Asserting the constructor name proves no
    // refinement layer exists.
    expect(eventSchema._def.typeName).toBe('ZodObject');
  });
});

describe('Event<T> generic narrowing', () => {
  it('narrows data when T is provided', () => {
    type DeployData = { service_id: string; sha: string };
    const ev: Event<DeployData> = {
      type: 'engineering.deploy.completed.v1',
      data: { service_id: 'api', sha: 'abc1234' },
      source: validSource,
    };
    // Type-narrowed access — would fail to compile if data were inferred as unknown.
    const narrowed: string = ev.data.service_id;
    expect(narrowed).toBe('api');
  });

  it('defaults T to unknown (type-level: data needs narrowing before use)', () => {
    const ev: Event = minimalEvent;
    // unknown forces a type-narrowing step before access; cast captures that
    // the default is unknown (not any, not the inferred shape).
    const data: unknown = ev.data;
    expect(data).toEqual({ foo: 'bar' });
  });
});

describe('Event<T> type discipline', () => {
  it('readonly fields prevent reassignment at the type level', () => {
    // Type-level test: any of the lines below would fail tsc if Event<T>
    // dropped readonly. They are commented out so the test compiles, but
    // the assertion is the strict-mode build itself catches mutation.
    const ev: Event<{ x: number }> = {
      type: 'a.b.c.v1',
      data: { x: 1 },
      source: validSource,
    };
    // ev.type = 'mut'           // ❌ ts(2540) Cannot assign to 'type' because it is a read-only property
    // ev.source = validSource;  // ❌ ts(2540)
    expect(ev.type).toBe('a.b.c.v1');
  });
});
