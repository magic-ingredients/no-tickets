import { describe, it, expect, vi } from 'vitest';
import { runPublishSingle, type PublishSingleDeps, type PublishSingleOptions } from './single.js';
import type { EventTypeSpec } from '../../../registry/client.js';
import type { PublishResponse, PublishEvent } from '../../../transport/events.js';

const TYPE_USER: EventTypeSpec = {
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
};

interface RecordedOutput {
  readonly stdout: string[];
  readonly stderr: string[];
}

interface BuildDepsOptions {
  readonly availableTypes: readonly EventTypeSpec[];
  readonly publishResult?: PublishResponse;
  readonly publishError?: unknown;
}

function buildDeps(opts: BuildDepsOptions, out: RecordedOutput): {
  deps: PublishSingleDeps;
  publish: ReturnType<typeof vi.fn>;
} {
  const publish = vi.fn<(events: readonly PublishEvent[]) => Promise<PublishResponse>>(
    async () => {
      if (opts.publishError !== undefined) throw opts.publishError;
      return opts.publishResult ?? { ingested: 1, deduped: 0, ids: ['evt_1'] };
    },
  );
  const deps: PublishSingleDeps = {
    listEvents: vi.fn(async () => opts.availableTypes),
    publish,
    readStdin: vi.fn(async () => ''),
    write: (line) => out.stdout.push(line),
    writeErr: (line) => out.stderr.push(line),
  };
  return { deps, publish };
}

const baseOptions = (typeId: string, data: string): PublishSingleOptions => ({
  typeId,
  data,
});

describe('runPublishSingle — happy path', () => {
  it('publishes a valid event and prints the ingested count and ids', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({ availableTypes: [TYPE_USER] }, out);

    const exit = await runPublishSingle(
      baseOptions('app.user.signed-up.v1', '{"email": "a@b.c"}'),
      deps,
    );

    expect(exit).toBe(0);
    expect(publish).toHaveBeenCalledTimes(1);
    const events = publish.mock.calls[0]?.[0];
    expect(events).toEqual([
      expect.objectContaining({
        type: 'app.user.signed-up.v1',
        data: { email: 'a@b.c' },
      }),
    ]);
    expect(out.stdout.join('\n')).toContain('1 event');
    expect(out.stdout.join('\n')).toContain('evt_1');
  });

  it('attaches subject when both --subject-type and --subject-id are provided', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({ availableTypes: [TYPE_USER] }, out);

    await runPublishSingle(
      {
        ...baseOptions('app.user.signed-up.v1', '{"email": "a@b.c"}'),
        subjectType: 'app.user',
        subjectId: 'usr_42',
      },
      deps,
    );

    const events = publish.mock.calls[0]?.[0];
    expect(events?.[0]).toMatchObject({
      subject: { type: 'app.user', id: 'usr_42' },
    });
  });

  it('omits subject when only one of subject-type/subject-id is provided', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({ availableTypes: [TYPE_USER] }, out);

    await runPublishSingle(
      {
        ...baseOptions('app.user.signed-up.v1', '{"email": "a@b.c"}'),
        subjectType: 'app.user',
      },
      deps,
    );

    const eventBody = publish.mock.calls[0]?.[0]?.[0];
    expect(eventBody?.subject).toBeUndefined();
  });

  it('attaches source overrides when source-name is provided', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({ availableTypes: [TYPE_USER] }, out);

    await runPublishSingle(
      {
        ...baseOptions('app.user.signed-up.v1', '{"email": "a@b.c"}'),
        sourceName: 'tiny-brain',
        sourceAttributes: ['env=prod'],
      },
      deps,
    );

    const eventBody = publish.mock.calls[0]?.[0]?.[0];
    expect(eventBody?.source).toMatchObject({
      name: 'tiny-brain',
      attributes: { env: 'prod' },
    });
  });
});

describe('runPublishSingle — unknown type id', () => {
  it('exits with code 2 and prints fuzzy-match suggestions to stderr', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({ availableTypes: [TYPE_USER] }, out);

    const exit = await runPublishSingle(
      baseOptions('app.user.signed-uppp.v1', '{"email": "a@b.c"}'),
      deps,
    );

    expect(exit).toBe(2);
    expect(publish).not.toHaveBeenCalled();
    const printed = out.stderr.join('\n');
    expect(printed).toContain('app.user.signed-up.v1'); // fuzzy suggestion
  });

  it('exits with code 2 even when there are no candidates in the cache', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({ availableTypes: [] }, out);

    const exit = await runPublishSingle(
      baseOptions('anything.v1', '{}'),
      deps,
    );

    expect(exit).toBe(2);
    expect(publish).not.toHaveBeenCalled();
  });
});

describe('runPublishSingle — local validation', () => {
  it('exits with code 1 and reports the field path when --data is missing a required field', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({ availableTypes: [TYPE_USER] }, out);

    const exit = await runPublishSingle(
      baseOptions('app.user.signed-up.v1', '{}'),
      deps,
    );

    expect(exit).toBe(1);
    expect(publish).not.toHaveBeenCalled();
    expect(out.stderr.join('\n')).toMatch(/email/i);
  });

  it('exits with code 1 when --data has a wrong-typed field', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({ availableTypes: [TYPE_USER] }, out);

    const exit = await runPublishSingle(
      baseOptions('app.user.signed-up.v1', '{"email": 42}'),
      deps,
    );

    expect(exit).toBe(1);
    expect(publish).not.toHaveBeenCalled();
    expect(out.stderr.join('\n')).toMatch(/email/);
  });

  it('rejects an enum violation with a "one of" message', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({ availableTypes: [TYPE_USER] }, out);

    const exit = await runPublishSingle(
      baseOptions('app.user.signed-up.v1', '{"email": "a@b.c", "plan": "enterprise"}'),
      deps,
    );

    expect(exit).toBe(1);
    expect(publish).not.toHaveBeenCalled();
    expect(out.stderr.join('\n')).toContain('plan');
    expect(out.stderr.join('\n')).toMatch(/one of/);
  });

  it('validates each item of an array schema; flags the failing index', async () => {
    const ARRAY_TYPE: EventTypeSpec = {
      ...TYPE_USER,
      id: 'app.list.v1',
      schema: {
        type: 'object',
        properties: {
          tags: { type: 'array', items: { type: 'string' } },
        },
        required: ['tags'],
      },
    };
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({ availableTypes: [ARRAY_TYPE] }, out);

    const exit = await runPublishSingle(
      baseOptions('app.list.v1', '{"tags": ["ok", 42]}'),
      deps,
    );

    expect(exit).toBe(1);
    expect(publish).not.toHaveBeenCalled();
    expect(out.stderr.join('\n')).toContain('tags.1');
  });

  it('rejects a non-array value at an array-typed field', async () => {
    const ARRAY_TYPE: EventTypeSpec = {
      ...TYPE_USER,
      id: 'app.list.v1',
      schema: {
        type: 'object',
        properties: {
          tags: { type: 'array', items: { type: 'string' } },
        },
      },
    };
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({ availableTypes: [ARRAY_TYPE] }, out);

    const exit = await runPublishSingle(
      baseOptions('app.list.v1', '{"tags": "not-an-array"}'),
      deps,
    );

    expect(exit).toBe(1);
    expect(publish).not.toHaveBeenCalled();
    expect(out.stderr.join('\n')).toMatch(/expected array/);
  });

  it.each([
    ['number', 'count', '{"count": 1}', '{"count": "1"}'],
    ['integer', 'n', '{"n": 1}', '{"n": "1"}'],
    ['boolean', 'flag', '{"flag": true}', '{"flag": 1}'],
    ['null', 'value', '{"value": null}', '{"value": 0}'],
  ])(
    'matchesType: %s field accepts the right value and rejects the wrong type',
    async (schemaType, field, ok, bad) => {
      const TYPE: EventTypeSpec = {
        ...TYPE_USER,
        id: 'app.x.v1',
        schema: {
          type: 'object',
          properties: { [field]: { type: schemaType as 'number' } },
        },
      };
      const okOut: RecordedOutput = { stdout: [], stderr: [] };
      const okDeps = buildDeps({ availableTypes: [TYPE] }, okOut);
      const okExit = await runPublishSingle(baseOptions('app.x.v1', ok), okDeps.deps);
      expect(okExit).toBe(0);

      const badOut: RecordedOutput = { stdout: [], stderr: [] };
      const badDeps = buildDeps({ availableTypes: [TYPE] }, badOut);
      const badExit = await runPublishSingle(baseOptions('app.x.v1', bad), badDeps.deps);
      expect(badExit).toBe(1);
      expect(badDeps.publish).not.toHaveBeenCalled();
    },
  );
});

describe('runPublishSingle — optional field omission on the wire', () => {
  it('omits parentEventId, traceId, dedupeKey, subject, and source when not supplied', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({ availableTypes: [TYPE_USER] }, out);

    await runPublishSingle(
      baseOptions('app.user.signed-up.v1', '{"email": "a@b.c"}'),
      deps,
    );

    const event = publish.mock.calls[0]?.[0]?.[0] ?? {};
    expect(event).not.toHaveProperty('parentEventId');
    expect(event).not.toHaveProperty('traceId');
    expect(event).not.toHaveProperty('dedupeKey');
    expect(event).not.toHaveProperty('subject');
    expect(event).not.toHaveProperty('source');
  });

  it('passes parentEventId, traceId, and dedupeKey through when supplied', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({ availableTypes: [TYPE_USER] }, out);

    await runPublishSingle(
      {
        ...baseOptions('app.user.signed-up.v1', '{"email": "a@b.c"}'),
        parent: 'evt_parent',
        trace: 'trace_xyz',
        dedupeKey: 'idempotency_1',
      },
      deps,
    );

    const event = publish.mock.calls[0]?.[0]?.[0];
    expect(event).toMatchObject({
      parentEventId: 'evt_parent',
      traceId: 'trace_xyz',
      dedupeKey: 'idempotency_1',
    });
  });
});

describe('runPublishSingle — input guards', () => {
  it('exits with code 1 when type id is empty (no listEvents call)', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({ availableTypes: [TYPE_USER] }, out);

    const exit = await runPublishSingle(baseOptions('', '{"email": "a@b.c"}'), deps);

    expect(exit).toBe(1);
    expect(deps.listEvents).not.toHaveBeenCalled();
    expect(publish).not.toHaveBeenCalled();
  });

  it('exits with code 1 when listEvents fails (cache missing)', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const publish = vi.fn();
    const deps: PublishSingleDeps = {
      listEvents: vi.fn(async () => {
        throw new Error('cache missing');
      }),
      publish,
      readStdin: vi.fn(async () => ''),
      write: (l) => out.stdout.push(l),
      writeErr: (l) => out.stderr.push(l),
    };

    const exit = await runPublishSingle(
      baseOptions('app.user.signed-up.v1', '{}'),
      deps,
    );

    expect(exit).toBe(1);
    expect(out.stderr.join('\n')).toContain('cache missing');
    expect(publish).not.toHaveBeenCalled();
  });

  it('exits with code 1 when --data fails to resolve (invalid JSON)', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({ availableTypes: [TYPE_USER] }, out);

    const exit = await runPublishSingle(
      baseOptions('app.user.signed-up.v1', '{not json'),
      deps,
    );

    expect(exit).toBe(1);
    expect(out.stderr.join('\n')).toMatch(/json/i);
    expect(publish).not.toHaveBeenCalled();
  });
});

describe('runPublishSingle — server error', () => {
  it('exits with code 3 when the server returns an error', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps(
      {
        availableTypes: [TYPE_USER],
        publishError: new Error('server boom'),
      },
      out,
    );

    const exit = await runPublishSingle(
      baseOptions('app.user.signed-up.v1', '{"email": "a@b.c"}'),
      deps,
    );

    expect(exit).toBe(3);
    expect(out.stderr.join('\n')).toContain('server boom');
  });
});
