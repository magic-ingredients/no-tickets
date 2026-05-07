import { z } from 'zod';

// -- Enum schemas (v1 — .notickets/ frontmatter) ------------------------------

export const phaseSchema = z.enum(['ideation', 'development', 'testing', 'review', 'done']);

export const entityStatusSchema = z.enum(['not_started', 'in_progress', 'completed']);

export const taskStatusSchema = z.enum(['not_started', 'in_progress', 'completed']);

export const assigneeTypeSchema = z.enum(['human', 'agent']);

export const documentTypeSchema = z.enum(['epic', 'feature', 'fix']);

// -- Date format --------------------------------------------------------------

const dateStringSchema = z.string().regex(/^\d{4}-\d{2}-\d{2}$/, 'Must be YYYY-MM-DD');

// -- Kebab-case ID ------------------------------------------------------------

const kebabIdSchema = z.string().regex(
  /^[a-z0-9]+(?:-[a-z0-9]+)*$/,
  'Must be kebab-case (lowercase letters, numbers, hyphens)',
);

// -- Frontmatter schemas ------------------------------------------------------

const baseFrontmatterFields = {
  id: kebabIdSchema,
  title: z.string().min(1, 'Title is required'),
  status: entityStatusSchema,
  created: dateStringSchema,
  updated: dateStringSchema,
  meta: z.record(z.string(), z.unknown()).optional(),
};

export const epicFrontmatterSchema = z.object({
  ...baseFrontmatterFields,
  type: z.literal('epic'),
});

export const featureFrontmatterSchema = z.object({
  ...baseFrontmatterFields,
  type: z.enum(['feature', 'fix']),
  epic: z.string().min(1, 'Epic reference is required'),
  phase: phaseSchema,
  assignee: z.string().optional(),
  assignee_type: assigneeTypeSchema.optional(),
});

// -- Task schema --------------------------------------------------------------

export const taskSchema = z.object({
  number: z.number().int().positive(),
  title: z.string().min(1, 'Task title is required'),
  status: taskStatusSchema,
});

