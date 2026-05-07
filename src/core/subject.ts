import { z } from 'zod';

export const subjectRefSchema = z.object({
  type: z.string().min(1),
  id: z.string().min(1),
});

export type SubjectRef = Readonly<{
  type: string;
  id: string;
}>;

export const subjectSchema = z.object({
  type: z.string().min(1),
  externalId: z.string().min(1),
  displayName: z.string().min(1),
  metadata: z.record(z.string(), z.unknown()).optional(),
});

// Explicit type — z.infer would only freeze the outer object, leaving
// metadata mutable. PRD requires deep-readonly metadata.
export type Subject = Readonly<{
  type: string;
  externalId: string;
  displayName: string;
  metadata?: Readonly<Record<string, unknown>>;
}>;
