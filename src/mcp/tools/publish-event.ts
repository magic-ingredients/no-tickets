import { z } from 'zod';
import type { ToolDescriptor } from './tool-descriptor.js';

export const publishEventTool: ToolDescriptor = {
  name: 'publish_event',
  description:
    'Publish a single event. Call describe_event_type first to confirm the ' +
    'schema; the server will reject mismatches. Source metadata is filled ' +
    'server-side and cannot be overridden.',
  inputSchema: {
    project: z
      .string()
      .min(1)
      .describe('Project name from the local registry (e.g. as set up via `nt project link`); routes the event to the matching account.'),
    type: z.string().min(1).describe('Type id (domain.entity.action.vN).'),
    data: z.record(z.string(), z.unknown()).describe('Event payload matching the type schema.'),
    subject: z
      .object({
        type: z.string().min(1),
        id: z.string().min(1),
      })
      .optional(),
    occurred_at: z.string().optional().describe('ISO-8601 timestamp; defaults to now server-side.'),
    parent_event_id: z.string().optional(),
    trace_id: z.string().optional(),
    dedupe_key: z.string().optional(),
  },
};
