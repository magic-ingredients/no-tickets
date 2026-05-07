import { z } from 'zod';

export const subjectRefSchema = z.object({
  type: z.string().min(1),
  id: z.string().min(1),
});

export type SubjectRef = Readonly<z.infer<typeof subjectRefSchema>>;
