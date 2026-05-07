import { z } from 'zod';
import type { ToolDescriptor } from './tool-descriptor.js';

export const listEventTypesTool: ToolDescriptor = {
  name: 'list_event_types',
  description:
    'List event types this caller can publish, optionally filtered by domain. ' +
    'Type ids follow domain.entity.action.vN grammar. Reads from the local ' +
    'cache; refresh fires async.',
  inputSchema: {
    domain: z.string().optional().describe('Filter to a single domain prefix.'),
    deprecated: z
      .boolean()
      .optional()
      .describe('When true, return ONLY deprecated types; when false, only active.'),
  },
};
