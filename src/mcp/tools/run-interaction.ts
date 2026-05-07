import { z } from 'zod';
import type { ToolDescriptor } from './tool-descriptor.js';

export const runInteractionTool: ToolDescriptor = {
  name: 'run_interaction',
  description:
    'Run a server-defined interaction by id with the given input. Returns ' +
    'the events it emitted. Use for compound actions where the server ' +
    'orchestrates multiple events.',
  inputSchema: {
    id: z.string().min(1).describe('Interaction id.'),
    input: z.record(z.string(), z.unknown()).describe('Input payload for the interaction.'),
    subject: z
      .object({
        type: z.string().min(1),
        id: z.string().min(1),
      })
      .optional(),
  },
};
