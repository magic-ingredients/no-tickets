import { z } from 'zod';
import { subjectRefSchema } from './subject.js';

export const interactionRequestSchema = z.object({
  id: z.string().min(1),
  input: z.unknown(),
  subject: subjectRefSchema.optional(),
});

// Mirrors Event<T>: derive from z.infer so schema additions flow into the type
// automatically, then narrow `input` via the generic. The PRD calls this
// Interaction<TInput>; the SDK splits Request/Response so the response shape
// has its own type rather than overloading one name.
export type InteractionRequest<TInput = unknown> = Readonly<
  Omit<z.infer<typeof interactionRequestSchema>, 'input'>
> & {
  readonly input: TInput;
};

export const interactionEventRefSchema = z.object({
  id: z.string().min(1),
  type: z.string().min(1),
});

export type InteractionEventRef = Readonly<z.infer<typeof interactionEventRefSchema>>;

export const interactionResponseSchema = z.object({
  events: z.array(interactionEventRefSchema),
});

export type InteractionResponse = Readonly<{
  events: readonly InteractionEventRef[];
}>;
