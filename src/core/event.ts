import { z } from 'zod';
import { sourceSchema } from './source.js';

export const subjectRefSchema = z.object({
  type: z.string().min(1),
  id: z.string().min(1),
});

export type SubjectRef = z.infer<typeof subjectRefSchema>;

export const eventSchema = z.object({
  type: z.string().min(1),
  data: z.unknown(),
  source: sourceSchema,
  subject: subjectRefSchema.optional(),
  occurredAt: z.string().min(1).optional(),
  parentEventId: z.string().min(1).optional(),
  traceId: z.string().min(1).optional(),
  dedupeKey: z.string().min(1).optional(),
});

export type Event<T = unknown> = Omit<z.infer<typeof eventSchema>, 'data'> & {
  data: T;
};
