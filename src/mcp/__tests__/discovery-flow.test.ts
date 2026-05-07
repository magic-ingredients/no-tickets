import { describe, it, expect, vi, expectTypeOf } from 'vitest';
import {
  handleListEventTypes,
  handleDescribeEventType,
  handlePublishEvent,
  type ToolHandlerDeps,
  type ListEventTypesArgs,
  type ListEventTypesResult,
  type DescribeEventTypeArgs,
  type DescribeEventTypeResult,
  type PublishEventArgs,
  type PublishEventResult,
  type RunInteractionArgs,
  type RunInteractionResult,
  type CreateSubjectArgs,
  type CreateSubjectResult,
  type ToolDescriptor,
  type StructuredToolError,
  type StructuredToolErrorCode,
  type StructuredToolFailure,
  type TransportHints,
} from '../discovery.js';
import * as discovery from '../discovery.js';
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
  // subjectsCreate / runInteraction are not exercised by the discovery flow
  // but ToolHandlerDeps requires them. Return-only stubs satisfy the shape;
  // call-count is asserted in the dedicated handlers.test.ts.
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

function firstTypeIdOrFail(listed: ListEventTypesResult): string {
  const target = listed.types[0];
  if (target === undefined) {
    throw new Error('discovery flow: list_event_types returned an empty array');
  }
  return target.id;
}

describe('MCP discovery flow — first event in three calls', () => {
  it('list → describe → publish_event: agent lands its first event with no prior knowledge of the registry', async () => {
    const { deps, publish } = buildIntegrationDeps();

    // 1. list_event_types — agent discovers what types are publishable.
    const listed = await handleListEventTypes({}, deps);
    expect(listed.types.length).toBeGreaterThan(0);
    const targetId = firstTypeIdOrFail(listed);
    expect(targetId).toBe(TYPE.id);

    // 2. describe_event_type — agent gets the schema + an example payload.
    const described = await handleDescribeEventType({ id: targetId }, deps);
    expect(described.schema).toEqual(TYPE.schema);
    expect(described.example).toBeDefined();

    // The synthesised example MUST pass local schema validation, otherwise
    // the agent's next step is doomed to a server reject.
    const validationErrors = validateAgainstSchema(described.example, described.schema);
    expect(validationErrors).toEqual([]);

    // 3. publish_event — agent uses the example as `data` verbatim.
    const result = await handlePublishEvent(
      {
        type: targetId,
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

  it('describe error halts the flow — handlePublishEvent is never invoked when describe rejects', async () => {
    // Stronger version: actually try to thread the (rejected) describe
    // result into publish. If publish runs at all it surfaces here, not just
    // through a vacuous "never called" check.
    const { deps, publish } = buildIntegrationDeps();

    const flow = async (): Promise<void> => {
      const described = await handleDescribeEventType({ id: 'does.not.exist.v1' }, deps);
      await handlePublishEvent(
        { type: 'does.not.exist.v1', data: described.example as Record<string, unknown> },
        deps,
      );
    };

    await expect(flow()).rejects.toThrow(/not found/i);
    expect(publish).not.toHaveBeenCalled();
  });

  it('discovery barrel exports the full RUNTIME surface (handlers, tool descriptors, helpers)', () => {
    const exposed = Object.keys(discovery).sort();
    expect(exposed).toEqual(
      [
        'createSubjectTool',
        'describeEventTypeTool',
        'handleCreateSubject',
        'handleDescribeEventType',
        'handleListEventTypes',
        'handlePublishEvent',
        'handleRunInteraction',
        'listEventTypesTool',
        'mapErrorToToolResult',
        'publishEventTool',
        'runInteractionTool',
        'sourceFromTransport',
      ],
    );
  });

  it('discovery barrel re-exports the TYPE-LEVEL surface (compile-time check)', () => {
    // Object.keys() can't see type-only re-exports. expectTypeOf assertions
    // still fail compilation if any of these aliases are dropped from the
    // barrel — embedders depend on them at compile time.
    expectTypeOf<ToolHandlerDeps>().not.toBeAny();
    expectTypeOf<ListEventTypesArgs>().not.toBeAny();
    expectTypeOf<ListEventTypesResult>().not.toBeAny();
    expectTypeOf<DescribeEventTypeArgs>().not.toBeAny();
    expectTypeOf<DescribeEventTypeResult>().not.toBeAny();
    expectTypeOf<PublishEventArgs>().not.toBeAny();
    expectTypeOf<PublishEventResult>().not.toBeAny();
    expectTypeOf<RunInteractionArgs>().not.toBeAny();
    expectTypeOf<RunInteractionResult>().not.toBeAny();
    expectTypeOf<CreateSubjectArgs>().not.toBeAny();
    expectTypeOf<CreateSubjectResult>().not.toBeAny();
    expectTypeOf<ToolDescriptor>().not.toBeAny();
    expectTypeOf<StructuredToolError>().not.toBeAny();
    expectTypeOf<StructuredToolErrorCode>().not.toBeAny();
    expectTypeOf<StructuredToolFailure>().not.toBeAny();
    expectTypeOf<TransportHints>().not.toBeAny();
  });
});
