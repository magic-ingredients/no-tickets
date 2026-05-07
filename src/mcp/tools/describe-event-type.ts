import { z } from 'zod';
import type { ToolDescriptor } from './tool-descriptor.js';

export const describeEventTypeTool: ToolDescriptor = {
  name: 'describe_event_type',
  description:
    'Return schema, dedupe strategy, retention, and a synthesised example ' +
    'payload for a single event type. Call this before publish_event when ' +
    'you do not already know the schema; the example field is a starting ' +
    'point you can adapt.',
  inputSchema: {
    id: z.string().min(1).describe('Type id (domain.entity.action.vN).'),
  },
};
