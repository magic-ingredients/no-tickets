import { z } from 'zod';

export const subjectRefSchema = z.object({
  type: z.string().min(1),
  id: z.string().min(1),
});

export type SubjectRef = Readonly<z.infer<typeof subjectRefSchema>>;

export const subjectSchema = z.object({
  type: z.string().min(1),
  externalId: z.string().min(1),
  displayName: z.string().min(1),
  metadata: z.record(z.string(), z.unknown()).optional(),
});

export type Subject = Readonly<z.infer<typeof subjectSchema>>;
