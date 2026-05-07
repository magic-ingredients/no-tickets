import { z } from 'zod';
import { subjectRefSchema, type SubjectRef } from './subject.js';

export const interactionRequestSchema = z.object({
  id: z.string().min(1),
  input: z.unknown(),
  subject: subjectRefSchema.optional(),
});

export type InteractionRequest<TInput = unknown> = Readonly<{
  id: string;
  input: TInput;
  subject?: SubjectRef;
}>;

export const interactionResponseSchema = z.object({
  events: z.array(
    z.object({
      id: z.string().min(1),
      type: z.string().min(1),
    }),
  ),
});

export type InteractionResponse = Readonly<{
  events: readonly Readonly<{ id: string; type: string }>[];
}>;
