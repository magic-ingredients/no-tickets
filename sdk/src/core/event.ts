import { z } from 'zod';
import { sourceSchema } from './source.js';

export const eventSchema = z.object({
  type: z.string().min(1),
  data: z.unknown(),
  source: sourceSchema,
  occurredAt: z.string().min(1).optional(),
  parentEventId: z.string().min(1).optional(),
  traceId: z.string().min(1).optional(),
  dedupeKey: z.string().min(1).optional(),
});

// Readonly enforces the PRD's immutability discipline at the type level —
// callers cannot mutate envelopes after construction.
export type Event<T = unknown> = Readonly<Omit<z.infer<typeof eventSchema>, 'data'>> & {
  readonly data: T;
};
