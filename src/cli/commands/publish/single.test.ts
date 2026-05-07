import { describe, it, expect, vi } from 'vitest';
import { runPublishSingle, type PublishSingleDeps, type PublishSingleOptions } from './single.js';
import type { EventTypeSpec } from '../../../registry/client.js';
import type { PublishResponse } from '../../../transport/events.js';

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
  const publish = vi.fn(async () => {
    if (opts.publishError !== undefined) throw opts.publishError;
    return opts.publishResult ?? { ingested: 1, deduped: 0, ids: ['evt_1'] };
  });
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
    const [_client, events] = publish.mock.calls[0] ?? [];
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

    const events = publish.mock.calls[0]?.[1];
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

    const eventBody = publish.mock.calls[0]?.[1]?.[0];
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

    const eventBody = publish.mock.calls[0]?.[1]?.[0];
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
