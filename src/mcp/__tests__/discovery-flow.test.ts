import { describe, it, expect, vi } from 'vitest';
import {
  handleListEventTypes,
  handleDescribeEventType,
  handlePublishEvent,
  type ToolHandlerDeps,
} from '../tools/handlers.js';
import type { EventTypeSpec } from '../../registry/client.js';
import type { Subject } from '../../core/subject.js';
import type { Source } from '../../core/source.js';
import type {
  PublishEvent,
  PublishResponse,
} from '../../transport/events.js';
import { validateAgainstSchema } from '../../cli/lib/schema-validate.js';

const SERVER_SOURCE: Source = {
  name: 'mcp',
  sdkVersion: '9.9.9-test',
  attributes: { client: 'claude-code', clientVersion: '1.2.3' },
};

const TYPE: EventTypeSpec = {
  id: 'app.user.signed-up.v1',
  domain: 'app.user',
  entity: 'user',
  action: 'signed-up',
  version: 1,
  schema: {
    type: 'object',
    properties: {
      email: { type: 'string' },
      plan: { type: 'string', enum: ['free', 'pro'] },
    },
    required: ['email'],
  },
  retentionDays: 90,
  dedupeStrategy: 'natural_key',
};

function buildIntegrationDeps(): {
  deps: ToolHandlerDeps;
  publish: ReturnType<typeof vi.fn>;
} {
  const publish = vi.fn(
    async (): Promise<PublishResponse> => ({
      ingested: 1,
      deduped: 0,
      ids: ['evt_first'],
    }),
  );
  const subjectsCreate = vi.fn(async (s: Subject): Promise<Subject> => s);
  const runInteraction = vi.fn(async () => ({ events: [] }));
  const deps: ToolHandlerDeps = {
    events: {
      list: vi.fn(async () => [TYPE]),
      describe: vi.fn(async (id: string) => (id === TYPE.id ? TYPE : null)),
    },
    subjectsCreate,
    publishEvents: publish,
    runInteraction,
    source: SERVER_SOURCE,
  };
  return { deps, publish };
}

describe('MCP discovery flow — first event in three calls', () => {
  it('list → describe → publish_event: agent lands its first event with no prior knowledge of the registry', async () => {
    const { deps, publish } = buildIntegrationDeps();

    // 1. list_event_types — agent discovers what types are publishable.
    const listed = await handleListEventTypes({}, deps);
    expect(listed.types.length).toBeGreaterThan(0);
    const targetId = listed.types[0]?.id;
    expect(targetId).toBe(TYPE.id);

    // 2. describe_event_type — agent gets the schema + an example payload.
    const described = await handleDescribeEventType({ id: targetId! }, deps);
    expect(described.schema).toEqual(TYPE.schema);
    expect(described.example).toBeDefined();

    // The synthesised example MUST pass local schema validation, otherwise
    // the agent's next step is doomed to a server reject.
    const validationErrors = validateAgainstSchema(described.example, described.schema);
    expect(validationErrors).toEqual([]);

    // 3. publish_event — agent uses the example as `data` verbatim.
    const result = await handlePublishEvent(
      {
        type: targetId!,
        data: described.example as Record<string, unknown>,
      },
      deps,
    );

    expect(result).toEqual({ id: 'evt_first', deduped: false });

    // The wire body carries the agent's data + the SERVER-side source.
    const sent = (publish.mock.calls[0]?.[0] as PublishEvent[])[0];
    expect(sent).toMatchObject({
      type: targetId,
      data: described.example,
      source: SERVER_SOURCE,
    });
  });

  it('the example payload reaches the wire body unchanged (no transformation between describe and publish)', async () => {
    const { deps, publish } = buildIntegrationDeps();

    const described = await handleDescribeEventType({ id: TYPE.id }, deps);
    await handlePublishEvent(
      { type: TYPE.id, data: described.example as Record<string, unknown> },
      deps,
    );

    const sent = (publish.mock.calls[0]?.[0] as PublishEvent[])[0];
    expect(sent?.data).toEqual(described.example);
  });

  it('agent cannot bypass discovery — describing an unknown id surfaces a clear error before publish is reached', async () => {
    const { deps, publish } = buildIntegrationDeps();

    await expect(
      handleDescribeEventType({ id: 'does.not.exist.v1' }, deps),
    ).rejects.toThrow(/not found/i);

    expect(publish).not.toHaveBeenCalled();
  });
});
