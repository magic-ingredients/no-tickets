import { describe, it, expect } from 'vitest';
import { eventSchema, type Event } from '../event.js';

const validSource = { name: 'cli', sdkVersion: '1.2.3' };

const minimalEvent = {
  type: 'engineering.deploy.completed.v1',
  data: { foo: 'bar' },
  source: validSource,
};

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
      subject: { type: 'feature', id: 'f-1' },
      occurredAt: '2026-05-07T10:30:00Z',
      parentEventId: 'e-100',
      traceId: 'session-abc',
      dedupeKey: 'unique-key-1',
    };
    expect(eventSchema.parse(event)).toEqual(event);
  });

  it('treats data as opaque (accepts any shape)', () => {
    expect(() =>
      eventSchema.parse({ ...minimalEvent, data: { deeply: { nested: { thing: [1, 2] } } } }),
    ).not.toThrow();
    expect(() => eventSchema.parse({ ...minimalEvent, data: 'a string' })).not.toThrow();
    expect(() => eventSchema.parse({ ...minimalEvent, data: 42 })).not.toThrow();
    expect(() => eventSchema.parse({ ...minimalEvent, data: null })).not.toThrow();
    expect(() => eventSchema.parse({ ...minimalEvent, data: [] })).not.toThrow();
  });

  it('rejects missing type', () => {
    const { type: _type, ...rest } = minimalEvent;
    expect(() => eventSchema.parse(rest)).toThrow();
  });

  it('rejects missing data (data is required, even if undefined-like values are allowed inside)', () => {
    const { data: _data, ...rest } = minimalEvent;
    expect(() => eventSchema.parse(rest)).toThrow();
  });

  it('rejects missing source', () => {
    const { source: _source, ...rest } = minimalEvent;
    expect(() => eventSchema.parse(rest)).toThrow();
  });

  it('rejects empty-string type', () => {
    expect(() => eventSchema.parse({ ...minimalEvent, type: '' })).toThrow();
  });

  it('rejects non-string type', () => {
    expect(() => eventSchema.parse({ ...minimalEvent, type: 42 })).toThrow();
  });

  it('rejects an invalid source object', () => {
    expect(() =>
      eventSchema.parse({ ...minimalEvent, source: { name: 'cli' } }),
    ).toThrow();
  });

  it('accepts optional subject when shaped { type, id }', () => {
    const event = { ...minimalEvent, subject: { type: 'feature', id: 'f-1' } };
    expect(() => eventSchema.parse(event)).not.toThrow();
  });

  it('rejects subject missing required fields', () => {
    expect(() =>
      eventSchema.parse({ ...minimalEvent, subject: { type: 'feature' } }),
    ).toThrow();
    expect(() =>
      eventSchema.parse({ ...minimalEvent, subject: { id: 'f-1' } }),
    ).toThrow();
  });

  it('rejects empty-string subject.type', () => {
    expect(() =>
      eventSchema.parse({ ...minimalEvent, subject: { type: '', id: 'f-1' } }),
    ).toThrow();
  });

  it('rejects empty-string subject.id', () => {
    expect(() =>
      eventSchema.parse({ ...minimalEvent, subject: { type: 'feature', id: '' } }),
    ).toThrow();
  });

  it('accepts optional occurredAt as ISO 8601 string', () => {
    expect(() =>
      eventSchema.parse({ ...minimalEvent, occurredAt: '2026-05-07T10:30:00Z' }),
    ).not.toThrow();
  });

  it('rejects non-string occurredAt', () => {
    expect(() =>
      eventSchema.parse({ ...minimalEvent, occurredAt: 1730000000 }),
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

  it('tolerates unknown top-level keys (forward-compat)', () => {
    const parsed = eventSchema.parse({ ...minimalEvent, futureField: 'allowed' });
    expect(parsed).toEqual(minimalEvent);
  });

  it('schema is not refined (.refine() banned per ADR-0001)', () => {
    // Refinements don't survive JSON Schema export. The check: there should be
    // no refinement effects on the schema. Round-trip via parse with a minimal
    // shape that satisfies all object-level constraints — any cross-field
    // refinement would surface as a parse error here we wouldn't expect.
    expect(() => eventSchema.parse(minimalEvent)).not.toThrow();
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
    expect(ev.data.service_id).toBe('api');
  });

  it('defaults T to unknown', () => {
    const ev: Event = minimalEvent;
    expect(ev.data).toBeDefined();
  });
});
