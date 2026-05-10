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
  version: 'v1',
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
  const describe = vi.fn(async (): Promise<EventTypeSpec | null> =>
    'describeResult' in opts ? (opts.describeResult ?? null) : TYPE,
  );
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
    const DEPLOY: EventTypeSpec = {
      id: 'engineering.deploy.completed.v1',
      domain: 'engineering',
      entity: 'deploy',
      action: 'completed',
      version: 'v1',
      schema: { type: 'object', properties: {} },
    };
    const { deps, list } = buildDeps({
      listResult: [TYPE, DEPLOY],
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
          version: 'v1',
        },
        {
          id: 'engineering.deploy.completed.v1',
          domain: 'engineering',
          entity: 'deploy',
          action: 'completed',
          version: 'v1',
        },
      ],
    });
  });

  it('omits filter keys that were not supplied (object has zero own keys)', async () => {
    // Vitest's `toHaveBeenCalledWith({})` treats { domain: undefined } as
    // equal to {}, so we can't rely on it to prove buildListOptions is
    // omitting keys. Inspect the actual call argument.
    const { deps, list } = buildDeps({});

    await handleListEventTypes({}, deps);

    const arg = list.mock.calls[0]?.[0];
    expect(Object.keys(arg ?? {})).toEqual([]);
  });

  it('forwards only the keys that are explicitly set', async () => {
    const { deps, list } = buildDeps({});

    await handleListEventTypes({ domain: 'app.user' }, deps);

    const arg = list.mock.calls[0]?.[0];
    expect(Object.keys(arg ?? {})).toEqual(['domain']);
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

  it('omits dedupe_strategy / retention_days / ui_hints / deprecated_at when the spec lacks them', async () => {
    const MINIMAL: EventTypeSpec = {
      id: 'app.minimal.v1',
      domain: 'app.minimal',
      entity: 'thing',
      action: 'happened',
      version: 'v1',
      schema: { type: 'object', properties: {} },
    };
    const { deps } = buildDeps({ describeResult: MINIMAL });

    const result = await handleDescribeEventType({ id: 'app.minimal.v1' }, deps);

    expect(result).not.toHaveProperty('dedupe_strategy');
    expect(result).not.toHaveProperty('retention_days');
    expect(result).not.toHaveProperty('ui_hints');
    expect(result).not.toHaveProperty('deprecated_at');
  });

  it('passes ui_hints through verbatim when the spec carries them', async () => {
    const WITH_HINTS: EventTypeSpec = {
      ...TYPE,
      uiHints: { color: 'green', icon: 'rocket' },
    };
    const { deps } = buildDeps({ describeResult: WITH_HINTS });

    const result = await handleDescribeEventType({ id: TYPE.id }, deps);

    expect(result.ui_hints).toEqual({ color: 'green', icon: 'rocket' });
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
    expect(sentEvents[0]).toMatchObject({
      type: 'app.user.signed-up.v1',
      data: { email: 'a@b.c' },
      source: MCP_SOURCE,
    });
    expect(result).toEqual({ id: 'evt_1', deduped: false });
  });

  it('an agent-supplied source on the args object is IGNORED — server source still wins', async () => {
    // Pin the security invariant at runtime, not just at the type level.
    // A malicious agent that bypasses input-schema validation cannot
    // forge a Source.
    const { deps, publish } = buildDeps({});

    const argsWithMaliciousSource = {
      type: 'app.user.signed-up.v1',
      data: { email: 'a@b.c' },
      source: { name: 'fake-agent', sdkVersion: '0', attributes: { client: 'attacker' } },
    } as unknown as Parameters<typeof handlePublishEvent>[0];

    await handlePublishEvent(argsWithMaliciousSource, deps);

    const sent = (publish.mock.calls[0]?.[0] as PublishEvent[])[0];
    expect(sent?.source).toEqual(MCP_SOURCE);
    // The agent's `source` key on args MUST NOT propagate to the wire body.
    expect(sent?.source).not.toMatchObject({ name: 'fake-agent' });
  });

  it('throws when the server response is missing the event id (contract violation)', async () => {
    const { deps } = buildDeps({
      publishResult: { ingested: 1, deduped: 0, ids: [] },
    });

    await expect(
      handlePublishEvent(
        { type: 'app.user.signed-up.v1', data: { email: 'a@b.c' } },
        deps,
      ),
    ).rejects.toThrow(/missing the event id/);
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

  it('reports deduped: false when ingested > 0 even if the response also has dedupes (mixed batch is impossible for a singular publish; defensive)', async () => {
    const { deps } = buildDeps({
      publishResult: { ingested: 1, deduped: 1, ids: ['evt_1'] },
    });

    const result = await handlePublishEvent(
      { type: 'app.user.signed-up.v1', data: { email: 'a@b.c' } },
      deps,
    );

    expect(result).toEqual({ id: 'evt_1', deduped: false });
  });

  it('reports deduped: false when both ingested and deduped are zero (boundary; kills > 0 → >= 0)', async () => {
    const { deps } = buildDeps({
      publishResult: { ingested: 0, deduped: 0, ids: ['evt_void'] },
    });

    const result = await handlePublishEvent(
      { type: 'app.user.signed-up.v1', data: { email: 'a@b.c' } },
      deps,
    );

    expect(result).toEqual({ id: 'evt_void', deduped: false });
  });
});

describe('handleRunInteraction', () => {
  it('forwards id and input strictly; returns events list', async () => {
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

    expect(runInt.mock.calls[0]?.[0]).toBe('app.thread.reply');
    // toEqual instead of toHaveBeenCalledWith — pin the exact wire body
    // shape, including the absence of stray keys.
    expect(runInt.mock.calls[0]?.[1]).toEqual({
      input: { text: 'hi' },
      subject: { type: 'app.user', id: 'usr_1' },
    });
    expect(result.events).toEqual([
      { id: 'evt_1', type: 'app.thread.replied.v1' },
      { id: 'evt_2', type: 'app.thread.notified.v1' },
    ]);
  });

  it('omits subject when not supplied — body forwards just { input }', async () => {
    const { deps, runInt } = buildDeps({});

    await handleRunInteraction(
      { id: 'app.thread.reply', input: { text: 'hi' } },
      deps,
    );

    expect(runInt.mock.calls[0]?.[1]).toEqual({ input: { text: 'hi' } });
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
