import { describe, it, expect, vi } from 'vitest';
import {
  handleListEventTypes,
  handleDescribeEventType,
  handlePublishEvent,
  handleRunInteraction,
  handleCreateSubject,
  type ToolHandlerDeps,
} from './handlers.js';
import type { EventTypeSpec } from '../../registry/client.js';
import type { Subject } from '../../core/subject.js';
import type { PublishEvent, PublishResponse } from '../../transport/events.js';
import type { Source } from '../../core/source.js';
import type { InteractionResponse } from '../../core/interaction.js';

const MCP_SOURCE: Source = {
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
  schema: { type: 'object', properties: { email: { type: 'string' } }, required: ['email'] },
  retentionDays: 90,
  dedupeStrategy: 'natural_key',
  deprecatedAt: null,
};

interface BuildDepsOptions {
  readonly listResult?: readonly EventTypeSpec[];
  readonly describeResult?: EventTypeSpec | null;
  readonly publishResult?: PublishResponse;
  readonly interactionResult?: InteractionResponse;
  readonly subjectResult?: Subject;
}

function buildDeps(opts: BuildDepsOptions = {}): {
  deps: ToolHandlerDeps;
  publish: ReturnType<typeof vi.fn>;
  describe: ReturnType<typeof vi.fn>;
  list: ReturnType<typeof vi.fn>;
  subjectsCreate: ReturnType<typeof vi.fn>;
  runInt: ReturnType<typeof vi.fn>;
} {
  const list = vi.fn(async () => opts.listResult ?? [TYPE]);
  const describe = vi.fn(async () => opts.describeResult ?? TYPE);
  const publish = vi.fn(
    async (): Promise<PublishResponse> =>
      opts.publishResult ?? { ingested: 1, deduped: 0, ids: ['evt_1'] },
  );
  const subjectsCreate = vi.fn(
    async (s: Subject): Promise<Subject> => opts.subjectResult ?? s,
  );
  const runInt = vi.fn(
    async (): Promise<InteractionResponse> =>
      opts.interactionResult ?? { events: [{ id: 'evt_1', type: 'app.x.v1' }] },
  );

  const deps: ToolHandlerDeps = {
    events: {
      list,
      describe,
    },
    subjectsCreate,
    publishEvents: publish,
    runInteraction: runInt,
    source: MCP_SOURCE,
  };
  return { deps, publish, describe, list, subjectsCreate, runInt };
}

describe('handleListEventTypes', () => {
  it('forwards filters to events.list and maps each spec to a summary tuple', async () => {
    const { deps, list } = buildDeps({
      listResult: [TYPE, { ...TYPE, id: 'engineering.deploy.completed.v1', domain: 'engineering' }],
    });

    const result = await handleListEventTypes(
      { domain: 'engineering', deprecated: false },
      deps,
    );

    expect(list).toHaveBeenCalledWith({ domain: 'engineering', deprecated: false });
    expect(result).toEqual({
      types: [
        {
          id: 'app.user.signed-up.v1',
          domain: 'app.user',
          entity: 'user',
          action: 'signed-up',
          version: 1,
        },
        {
          id: 'engineering.deploy.completed.v1',
          domain: 'engineering',
          entity: 'deploy',
          action: 'completed',
          version: 1,
        },
      ],
    });
  });

  it('omits filters that were not supplied (empty options forwarded as {})', async () => {
    const { deps, list } = buildDeps({});

    await handleListEventTypes({}, deps);

    expect(list).toHaveBeenCalledWith({});
  });
});

describe('handleDescribeEventType', () => {
  it('returns the spec fields plus a synthesised example payload', async () => {
    const { deps } = buildDeps({});

    const result = await handleDescribeEventType({ id: 'app.user.signed-up.v1' }, deps);

    expect(result).toMatchObject({
      id: 'app.user.signed-up.v1',
      schema: TYPE.schema,
      dedupe_strategy: 'natural_key',
      retention_days: 90,
      deprecated_at: null,
    });
    // Example is synthesised from the schema; for { email: string, required: ['email'] }
    // we expect { email: '' }.
    expect(result.example).toEqual({ email: '' });
  });

  it('throws when the type id is not found', async () => {
    const { deps } = buildDeps({ describeResult: null });

    await expect(
      handleDescribeEventType({ id: 'app.unknown.v1' }, deps),
    ).rejects.toThrow(/not found/i);
  });
});

describe('handlePublishEvent', () => {
  it('arrives at publishEvents with the SERVER-SIDE source attached, not anything an agent could supply', async () => {
    const { deps, publish } = buildDeps({});

    const result = await handlePublishEvent(
      {
        type: 'app.user.signed-up.v1',
        data: { email: 'a@b.c' },
      },
      deps,
    );

    expect(publish).toHaveBeenCalledTimes(1);
    const sentEvents = publish.mock.calls[0]?.[0] as PublishEvent[];
    expect(sentEvents).toHaveLength(1);
    expect(sentEvents[0]?.source).toEqual(MCP_SOURCE);
    expect(result).toEqual({ id: 'evt_1', deduped: false });
  });

  it('passes through optional fields when supplied', async () => {
    const { deps, publish } = buildDeps({});

    await handlePublishEvent(
      {
        type: 'app.user.signed-up.v1',
        data: { email: 'a@b.c' },
        subject: { type: 'app.user', id: 'usr_1' },
        occurred_at: '2026-01-01T00:00:00Z',
        parent_event_id: 'evt_parent',
        trace_id: 'trace_xyz',
        dedupe_key: 'idem_1',
      },
      deps,
    );

    const sent = (publish.mock.calls[0]?.[0] as PublishEvent[])[0];
    expect(sent).toMatchObject({
      type: 'app.user.signed-up.v1',
      data: { email: 'a@b.c' },
      subject: { type: 'app.user', id: 'usr_1' },
      occurredAt: '2026-01-01T00:00:00Z',
      parentEventId: 'evt_parent',
      traceId: 'trace_xyz',
      dedupeKey: 'idem_1',
    });
  });

  it('omits optional fields entirely when not supplied (no `subject: undefined` keys)', async () => {
    const { deps, publish } = buildDeps({});

    await handlePublishEvent(
      { type: 'app.user.signed-up.v1', data: { email: 'a@b.c' } },
      deps,
    );

    const sent = (publish.mock.calls[0]?.[0] as PublishEvent[])[0] ?? {};
    expect(sent).not.toHaveProperty('subject');
    expect(sent).not.toHaveProperty('occurredAt');
    expect(sent).not.toHaveProperty('parentEventId');
    expect(sent).not.toHaveProperty('traceId');
    expect(sent).not.toHaveProperty('dedupeKey');
  });

  it('reports deduped: true when the server response shows ingested === 0 with deduped >= 1', async () => {
    const { deps } = buildDeps({
      publishResult: { ingested: 0, deduped: 1, ids: ['evt_dup'] },
    });

    const result = await handlePublishEvent(
      { type: 'app.user.signed-up.v1', data: { email: 'a@b.c' } },
      deps,
    );

    expect(result).toEqual({ id: 'evt_dup', deduped: true });
  });
});

describe('handleRunInteraction', () => {
  it('forwards id and input; returns events list', async () => {
    const { deps, runInt } = buildDeps({
      interactionResult: {
        events: [
          { id: 'evt_1', type: 'app.thread.replied.v1' },
          { id: 'evt_2', type: 'app.thread.notified.v1' },
        ],
      },
    });

    const result = await handleRunInteraction(
      {
        id: 'app.thread.reply',
        input: { text: 'hi' },
        subject: { type: 'app.user', id: 'usr_1' },
      },
      deps,
    );

    expect(runInt).toHaveBeenCalledWith('app.thread.reply', {
      input: { text: 'hi' },
      subject: { type: 'app.user', id: 'usr_1' },
    });
    expect(result.events).toEqual([
      { id: 'evt_1', type: 'app.thread.replied.v1' },
      { id: 'evt_2', type: 'app.thread.notified.v1' },
    ]);
  });

  it('omits subject when not supplied', async () => {
    const { deps, runInt } = buildDeps({});

    await handleRunInteraction(
      { id: 'app.thread.reply', input: { text: 'hi' } },
      deps,
    );

    const arg = runInt.mock.calls[0]?.[1];
    expect(arg).not.toHaveProperty('subject');
  });
});

describe('handleCreateSubject', () => {
  it('forwards a constructed Subject to subjects.create and returns { type, id }', async () => {
    const { deps, subjectsCreate } = buildDeps({});

    const result = await handleCreateSubject(
      {
        type: 'app.user',
        external_id: 'usr_1',
        display_name: 'Ada',
        metadata: { plan: 'pro' },
      },
      deps,
    );

    expect(subjectsCreate).toHaveBeenCalledWith({
      type: 'app.user',
      externalId: 'usr_1',
      displayName: 'Ada',
      metadata: { plan: 'pro' },
    });
    expect(result).toEqual({ type: 'app.user', id: 'usr_1' });
  });

  it('omits metadata when not supplied', async () => {
    const { deps, subjectsCreate } = buildDeps({});

    await handleCreateSubject(
      { type: 'app.user', external_id: 'usr_1', display_name: 'Ada' },
      deps,
    );

    const arg = subjectsCreate.mock.calls[0]?.[0];
    expect(arg).not.toHaveProperty('metadata');
  });
});
