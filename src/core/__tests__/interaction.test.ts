import { describe, it, expect } from 'vitest';
import {
  interactionRequestSchema,
  interactionResponseSchema,
  interactionEventRefSchema,
  type InteractionRequest,
  type InteractionResponse,
} from '../interaction.js';
import { subjectRefSchema } from '../subject.js';

const minimalRequest = {
  id: 'engineering.health.probe_requested.v1',
  input: { service_id: 'api' },
};

describe('interactionRequestSchema', () => {
  it('accepts the minimal valid request', () => {
    const parsed = interactionRequestSchema.parse(minimalRequest);
    expect(parsed).toEqual(minimalRequest);
  });

  it('accepts a request with subject (round-trip)', () => {
    const req = { ...minimalRequest, subject: { type: 'feature', id: 'f-1' } };
    const parsed = interactionRequestSchema.parse(req);
    expect(parsed).toEqual(req);
    expect(parsed.subject).toEqual({ type: 'feature', id: 'f-1' });
  });

  it('treats input as opaque (any shape)', () => {
    const cases: unknown[] = [
      { x: 1 },
      'a string',
      42,
      null,
      [],
      true,
      false,
      undefined,
    ];
    for (const input of cases) {
      expect(() => interactionRequestSchema.parse({ ...minimalRequest, input })).not.toThrow();
    }
  });

  it('tolerates missing input (z.unknown() — per-interaction schema validates server-side)', () => {
    const { input, ...rest } = minimalRequest;
    void input;
    expect(() => interactionRequestSchema.parse(rest)).not.toThrow();
  });

  it('rejects missing id', () => {
    const { id, ...rest } = minimalRequest;
    void id;
    expect(() => interactionRequestSchema.parse(rest)).toThrow();
  });

  it('rejects empty-string id', () => {
    expect(() => interactionRequestSchema.parse({ ...minimalRequest, id: '' })).toThrow();
  });

  it('rejects non-string id', () => {
    expect(() => interactionRequestSchema.parse({ ...minimalRequest, id: 42 })).toThrow();
  });

  it('delegates subject validation to subjectRefSchema (reference equality)', () => {
    expect(interactionRequestSchema.shape.subject.unwrap()).toBe(subjectRefSchema);
  });

  it('rejects subject missing type', () => {
    expect(() =>
      interactionRequestSchema.parse({ ...minimalRequest, subject: { id: 'f-1' } }),
    ).toThrow();
  });

  it('rejects subject missing id', () => {
    expect(() =>
      interactionRequestSchema.parse({ ...minimalRequest, subject: { type: 'feature' } }),
    ).toThrow();
  });

  it('rejects subject with empty-string type', () => {
    expect(() =>
      interactionRequestSchema.parse({ ...minimalRequest, subject: { type: '', id: 'f-1' } }),
    ).toThrow();
  });

  it('rejects subject with empty-string id', () => {
    expect(() =>
      interactionRequestSchema.parse({ ...minimalRequest, subject: { type: 'feature', id: '' } }),
    ).toThrow();
  });

  it('tolerates unknown top-level keys (forward-compat)', () => {
    const parsed = interactionRequestSchema.parse({ ...minimalRequest, futureField: 'allowed' });
    expect(parsed).toEqual(minimalRequest);
    expect(Object.keys(parsed)).not.toContain('futureField');
  });

  it('rejects non-object root', () => {
    expect(() => interactionRequestSchema.parse(null)).toThrow();
    expect(() => interactionRequestSchema.parse('string')).toThrow();
    expect(() => interactionRequestSchema.parse(42)).toThrow();
    expect(() => interactionRequestSchema.parse([])).toThrow();
  });

  it('schema is not a ZodEffects (.refine() banned per ADR-0001)', () => {
    expect(interactionRequestSchema._def.typeName).toBe('ZodObject');
  });

  it('InteractionRequest<TInput> narrows input', () => {
    type ProbeInput = { service_id: string };
    const req: InteractionRequest<ProbeInput> = {
      id: 'engineering.health.probe_requested.v1',
      input: { service_id: 'api' },
    };
    const narrowed: string = req.input.service_id;
    expect(narrowed).toBe('api');
  });

  it('InteractionRequest enforces readonly fields at compile time', () => {
    const req: InteractionRequest = minimalRequest;
    // @ts-expect-error — readonly
    req.id = 'mut';
    // @ts-expect-error — readonly
    req.input = 'mut';
    // @ts-expect-error — readonly
    req.subject = { type: 'mut', id: 'mut' };
    expect(req).toBeDefined();
  });
});

const minimalResponse = {
  events: [{ id: 'e-1', type: 'engineering.health.checked.v1' }],
};

describe('interactionResponseSchema', () => {
  it('accepts the minimal valid response', () => {
    const parsed = interactionResponseSchema.parse(minimalResponse);
    expect(parsed).toEqual(minimalResponse);
  });

  it('accepts an empty events array', () => {
    const parsed = interactionResponseSchema.parse({ events: [] });
    expect(parsed.events).toEqual([]);
  });

  it('accepts multiple events', () => {
    const response = {
      events: [
        { id: 'e-1', type: 'engineering.health.checked.v1' },
        { id: 'e-2', type: 'engineering.alert.fired.v1' },
      ],
    };
    expect(interactionResponseSchema.parse(response)).toEqual(response);
  });

  it('rejects missing events array', () => {
    expect(() => interactionResponseSchema.parse({})).toThrow();
  });

  it('rejects non-array events', () => {
    expect(() => interactionResponseSchema.parse({ events: 'not an array' })).toThrow();
    expect(() => interactionResponseSchema.parse({ events: {} })).toThrow();
  });

  it('rejects events with missing id', () => {
    expect(() =>
      interactionResponseSchema.parse({ events: [{ type: 'engineering.health.checked.v1' }] }),
    ).toThrow();
  });

  it('rejects events with missing type', () => {
    expect(() => interactionResponseSchema.parse({ events: [{ id: 'e-1' }] })).toThrow();
  });

  it('rejects events with empty-string id', () => {
    expect(() =>
      interactionResponseSchema.parse({ events: [{ id: '', type: 'a.b.c.v1' }] }),
    ).toThrow();
  });

  it('rejects events with empty-string type', () => {
    expect(() =>
      interactionResponseSchema.parse({ events: [{ id: 'e-1', type: '' }] }),
    ).toThrow();
  });

  it('delegates inner event-item validation to interactionEventRefSchema (reference equality)', () => {
    // .array() wraps; .element exposes the item schema.
    expect(interactionResponseSchema.shape.events.element).toBe(interactionEventRefSchema);
  });

  it('inner event-item schema is not a ZodEffects', () => {
    expect(interactionEventRefSchema._def.typeName).toBe('ZodObject');
  });

  it('tolerates unknown top-level keys on the response (forward-compat)', () => {
    const parsed = interactionResponseSchema.parse({ ...minimalResponse, futureField: 'allowed' });
    expect(parsed).toEqual(minimalResponse);
    expect(Object.keys(parsed)).not.toContain('futureField');
  });

  it('strips unknown keys on inner event items (forward-compat)', () => {
    const parsed = interactionResponseSchema.parse({
      events: [{ id: 'e-1', type: 'a.b.c.v1', futureField: 'allowed' }],
    });
    expect(parsed.events[0]).toEqual({ id: 'e-1', type: 'a.b.c.v1' });
  });

  it('rejects non-object root', () => {
    expect(() => interactionResponseSchema.parse(null)).toThrow();
    expect(() => interactionResponseSchema.parse('string')).toThrow();
    expect(() => interactionResponseSchema.parse(42)).toThrow();
    expect(() => interactionResponseSchema.parse([])).toThrow();
  });

  it('schema is not a ZodEffects (.refine() banned per ADR-0001)', () => {
    expect(interactionResponseSchema._def.typeName).toBe('ZodObject');
  });

  it('InteractionResponse enforces readonly events at compile time', () => {
    const resp: InteractionResponse = minimalResponse;
    // @ts-expect-error — readonly array property
    resp.events = [];
    // @ts-expect-error — readonly array (no push)
    resp.events.push({ id: 'e-x', type: 'a.b.c.v1' });
    // @ts-expect-error — inner items are readonly
    resp.events[0]!.id = 'mut';
    // @ts-expect-error — inner items are readonly
    resp.events[0]!.type = 'mut';
    expect(resp).toBeDefined();
  });
});
