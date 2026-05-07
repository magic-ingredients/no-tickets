import { describe, it, expect } from 'vitest';
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
    expect(names).toEqual(
      ['create_subject', 'describe_event_type', 'list_event_types', 'publish_event', 'run_interaction'].sort(),
    );
    expect(names).not.toContain('publish_events');
    expect(names).not.toContain('push');
  });

  it.each(tools.map((t) => [t.name, t]))(
    'tool %s description is at most 60 words',
    (_name, tool) => {
      const wordCount = tool.description.trim().split(/\s+/).length;
      expect(wordCount).toBeLessThanOrEqual(MAX_DESCRIPTION_WORDS);
    },
  );

  it('list_event_types description names the type-id grammar', () => {
    expect(listEventTypesTool.description.toLowerCase()).toMatch(/domain.*entity.*action.*version|type.id|type-id/);
  });

  it('describe_event_type description tells agents to call before publish_event', () => {
    expect(describeEventTypeTool.description).toContain('publish_event');
  });

  it('publish_event description references describe_event_type as the prerequisite', () => {
    expect(publishEventTool.description).toContain('describe_event_type');
  });

  it('publish_event input schema does NOT accept a `source` field (server-side only)', () => {
    // The schema's keys are the agent-supplied input fields. Source is filled
    // by the MCP server, not the agent.
    expect(Object.keys(publishEventTool.inputSchema)).not.toContain('source');
  });
});
