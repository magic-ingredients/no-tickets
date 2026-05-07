import { describe, it, expect } from 'vitest';
import { z } from 'zod';
import { listEventTypesTool } from './list-event-types.js';
import { describeEventTypeTool } from './describe-event-type.js';
import { publishEventTool } from './publish-event.js';
import { runInteractionTool } from './run-interaction.js';
import { createSubjectTool } from './create-subject.js';
import type { ToolDescriptor } from './tool-descriptor.js';

const tools: readonly ToolDescriptor[] = [
  listEventTypesTool,
  describeEventTypeTool,
  publishEventTool,
  runInteractionTool,
  createSubjectTool,
];

const MAX_DESCRIPTION_WORDS = 60;

describe('MCP discovery tools — registration shape', () => {
  it('exports exactly five tools (no publish_events plural)', () => {
    expect(tools).toHaveLength(5);
    const names = tools.map((t) => t.name).sort();
    expect(names).toEqual([
      'create_subject',
      'describe_event_type',
      'list_event_types',
      'publish_event',
      'run_interaction',
    ]);
    expect(names).not.toContain('publish_events');
    expect(names).not.toContain('push');
  });

  it.each(tools.map((t) => [t.name, t] as const))(
    'tool %s description is at most 60 words AND under 500 characters',
    (_name, tool) => {
      const wordCount = tool.description.trim().split(/\s+/).length;
      expect(wordCount).toBeLessThanOrEqual(MAX_DESCRIPTION_WORDS);
      // Char budget complements the word count — dotted/hyphenated lumps
      // count as one word but eat real context bytes.
      expect(tool.description.length).toBeLessThanOrEqual(500);
    },
  );

  it('list_event_types description names the type-id grammar segments', () => {
    const desc = listEventTypesTool.description.toLowerCase();
    expect(desc).toContain('domain');
    expect(desc).toContain('entity');
    expect(desc).toContain('action');
  });

  it('describe_event_type description tells agents to call before publish_event AND mentions the example field', () => {
    expect(describeEventTypeTool.description).toContain('publish_event');
    expect(describeEventTypeTool.description.toLowerCase()).toContain('example');
  });

  it('publish_event description references describe_event_type as the prerequisite', () => {
    expect(publishEventTool.description).toContain('describe_event_type');
  });

  it('publish_event input schema does NOT accept a `source` field (server-side only)', () => {
    // The schema's keys are the agent-supplied input fields. Source is filled
    // by the MCP server, not the agent.
    expect(Object.keys(publishEventTool.inputSchema)).not.toContain('source');
  });

  it('publish_event REJECTS an agent-supplied source field at parse time (strict mode)', () => {
    // Build a strict object schema from the descriptor so a `source` key
    // would fail validation rather than slip through.
    const schema = z.object(publishEventTool.inputSchema).strict();
    const result = schema.safeParse({
      type: 'app.thread.replied.v1',
      data: { text: 'hi' },
      source: { name: 'malicious-agent', sdkVersion: '0' },
    });

    expect(result.success).toBe(false);
    if (!result.success) {
      // zod's strict-mode rejection surfaces as an `unrecognized_keys` issue
      // listing the offending key(s). Either path-based or keys-based check
      // proves the field was the trigger.
      const flagged = result.error.issues.some(
        (i) =>
          i.path.includes('source') ||
          ('keys' in i && Array.isArray((i as { keys?: unknown[] }).keys) &&
            ((i as { keys: unknown[] }).keys).includes('source')),
      );
      expect(flagged).toBe(true);
    }
  });
});
