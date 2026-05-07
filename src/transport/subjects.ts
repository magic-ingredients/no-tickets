import { z } from 'zod';
import type { Client } from './client.js';
import {
  subjectSchema,
  subjectRefSchema,
  type Subject,
  type SubjectRef,
} from '../core/subject.js';

const subjectListSchema = z.array(subjectSchema);
const listQuerySchema = z.object({ type: z.string().min(1) });

export type SubjectListQuery = Readonly<z.infer<typeof listQuerySchema>>;

export const subjects = {
  async create(client: Client, subject: Subject): Promise<Subject> {
    subjectSchema.parse(subject);
    const response = await client.request<unknown>('POST', '/v1/subjects', subject);
    return subjectSchema.parse(response);
  },

  async get(client: Client, ref: SubjectRef): Promise<Subject> {
    subjectRefSchema.parse(ref);
    const path = `/v1/subjects/${encodeURIComponent(ref.type)}/${encodeURIComponent(ref.id)}`;
    const response = await client.request<unknown>('GET', path);
    return subjectSchema.parse(response);
  },

  async list(client: Client, query: SubjectListQuery): Promise<readonly Subject[]> {
    listQuerySchema.parse(query);
    const path = `/v1/subjects?${new URLSearchParams({ type: query.type }).toString()}`;
    const response = await client.request<unknown>('GET', path);
    return subjectListSchema.parse(response);
  },
};
