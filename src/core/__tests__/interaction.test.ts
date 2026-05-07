import { describe, it, expect } from 'vitest';
import {
  interactionRequestSchema,
  interactionResponseSchema,
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

  it('accepts a request with subject', () => {
    const req = { ...minimalRequest, subject: { type: 'feature', id: 'f-1' } };
    expect(interactionRequestSchema.parse(req)).toEqual(req);
  });

  it('treats input as opaque (any shape)', () => {
    const cases: unknown[] = [
      { x: 1 },
      'a string',
      42,
      null,
      [],
      true,
      undefined,
    ];
    for (const input of cases) {
      expect(() => interactionRequestSchema.parse({ ...minimalRequest, input })).not.toThrow();
    }
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

  it('rejects invalid subject (missing fields)', () => {
    expect(() =>
      interactionRequestSchema.parse({ ...minimalRequest, subject: { type: 'feature' } }),
    ).toThrow();
  });

  it('delegates subject validation to subjectRefSchema (reference equality)', () => {
    expect(interactionRequestSchema.shape.subject.unwrap()).toBe(subjectRefSchema);
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

  it('rejects events with empty-string id or type', () => {
    expect(() =>
      interactionResponseSchema.parse({ events: [{ id: '', type: 'a.b.c.v1' }] }),
    ).toThrow();
    expect(() =>
      interactionResponseSchema.parse({ events: [{ id: 'e-1', type: '' }] }),
    ).toThrow();
  });

  it('schema is not a ZodEffects (.refine() banned per ADR-0001)', () => {
    expect(interactionResponseSchema._def.typeName).toBe('ZodObject');
  });

  it('InteractionResponse enforces readonly events at compile time', () => {
    const resp: InteractionResponse = minimalResponse;
    // @ts-expect-error — readonly array
    resp.events = [];
    expect(resp).toBeDefined();
  });
});
