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

// =============================================================================
// Push Payload v2 — Zod schemas for runtime validation.
// Matches the TypeScript types in types.ts exactly.
// =============================================================================

const metaSchema = z.record(z.string(), z.unknown()).optional();

// -- Enum schemas (v2 — push payload) -----------------------------------------

export const workEntityTypeSchema = z.enum(['epic', 'feature', 'task']);

export const engineeringPhaseSchema = z.enum(['red', 'green', 'refactor', 'review', 'complete']);

export const acceptanceStatusSchema = z.enum(['unreviewed', 'accepted', 'changes_requested']);

export const prioritySchema = z.enum(['critical', 'high', 'medium', 'low']);

export const codeQualitySourceSchema = z.enum(['local', 'ci']);

// -- Core envelope ------------------------------------------------------------

export const pushEnvironmentSchema = z.object({
  os: z.string().optional(),
  runtime: z.string().optional(),
  ci: z.boolean().optional(),
  ciProvider: z.string().optional(),
});

export const sessionSchema = z.object({
  agent: z.string(),
  agentType: assigneeTypeSchema,
  model: z.string().optional(),
  vendor: z.string().optional(),
  environment: pushEnvironmentSchema.optional(),
  duration: z.number().nonnegative().optional(),
  result: z.string().optional(),
  meta: metaSchema,
});

// -- Schema: "work" -----------------------------------------------------------

export const workEntitySchema = z.object({
  id: z.string(),
  type: workEntityTypeSchema,
  parentId: z.string().optional(),
  title: z.string(),
  status: entityStatusSchema,
  assignee: z.string().optional(),
  assigneeType: assigneeTypeSchema.optional(),
  meta: metaSchema,
});

export const workDataSchema = z.object({
  entities: z.array(workEntitySchema),
});

// -- Schema: "engineering" ----------------------------------------------------

export const engineeringReviewSchema = z.object({
  reviewer: z.string(),
  verdict: z.string(),
  findings: z.number().optional(),
});

export const engineeringTaskSchema = z.object({
  entityId: z.string(),
  phase: engineeringPhaseSchema.optional(),
  commitSha: z.string().optional(),
  startedAt: z.string().optional(),
  completedAt: z.string().optional(),
  duration: z.number().nonnegative().optional(),
  reviews: z.array(engineeringReviewSchema).optional(),
  meta: metaSchema,
});

export const engineeringDataSchema = z.object({
  tasks: z.array(engineeringTaskSchema).optional(),
  meta: metaSchema,
});

// -- Schema: "product" --------------------------------------------------------

export const productUpdateSchema = z.object({
  entityId: z.string(),
  acceptance: acceptanceStatusSchema.optional(),
  priority: prioritySchema.optional(),
  labels: z.array(z.string()).optional(),
  releaseId: z.string().optional(),
  notes: z.string().optional(),
  meta: metaSchema,
});

export const productDataSchema = z.object({
  updates: z.array(productUpdateSchema),
  meta: metaSchema,
});

// -- Schema: "codeQuality" ----------------------------------------------------

export const codeQualityDataSchema = z.object({
  score: z.number().finite().nonnegative(),
  grade: z.string().optional(),
  source: codeQualitySourceSchema.optional(),
  entityId: z.string().optional(),
  categories: z.record(z.string(), z.number()).optional(),
  meta: metaSchema,
});

// -- Push envelope ------------------------------------------------------------

export const pushSchema = z.object({
  projectId: z.string(),
  timestamp: z.string(),
  session: sessionSchema.optional(),
  work: workDataSchema.optional(),
  engineering: engineeringDataSchema.optional(),
  product: productDataSchema.optional(),
  codeQuality: codeQualityDataSchema.optional(),
  custom: z.record(z.string(), z.unknown()).optional(),
});
