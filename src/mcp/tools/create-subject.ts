import { z } from 'zod';
import type { ToolDescriptor } from './tool-descriptor.js';

export const createSubjectTool: ToolDescriptor = {
  name: 'create_subject',
  description:
    'Promote an external identifier to a tracked subject. Idempotent: ' +
    'creating an existing subject returns the existing record.',
  inputSchema: {
    type: z.string().min(1).describe('Subject type, e.g. "app.user".'),
    external_id: z.string().min(1).describe('Caller-side identifier.'),
    display_name: z.string().min(1),
    metadata: z
      .record(z.string(), z.unknown())
      .optional()
      .describe('Optional structured metadata.'),
  },
};
