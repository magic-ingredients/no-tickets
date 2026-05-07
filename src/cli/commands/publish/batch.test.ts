import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import {
  runPublishBatch,
  type PublishBatchDeps,
  type PublishBatchOptions,
} from './batch.js';
import type { EventTypeSpec } from '../../../registry/client.js';
import type { PublishEvent, PublishResponse } from '../../../transport/events.js';
import { UnknownEventTypeError } from '../../../transport/errors.js';

const TYPE: EventTypeSpec = {
  id: 'app.user.signed-up.v1',
  domain: 'app.user',
  entity: 'user',
  action: 'signed-up',
  version: 1,
  schema: {
    type: 'object',
    properties: { email: { type: 'string' } },
    required: ['email'],
  },
};

interface RecordedOutput {
  readonly stdout: string[];
  readonly stderr: string[];
}

interface BuildBatchDepsOpts {
  readonly availableTypes: readonly EventTypeSpec[];
  readonly publishResult?: PublishResponse;
  readonly publishError?: unknown;
}

function buildDeps(opts: BuildBatchDepsOpts, out: RecordedOutput): {
  deps: PublishBatchDeps;
  publish: ReturnType<typeof vi.fn>;
} {
  const publish = vi.fn<(events: readonly PublishEvent[]) => Promise<PublishResponse>>(
    async () => {
      if (opts.publishError !== undefined) throw opts.publishError;
      return opts.publishResult ?? { ingested: 1, deduped: 0, ids: ['evt_1'] };
    },
  );
  const deps: PublishBatchDeps = {
    listEvents: vi.fn(async () => opts.availableTypes),
    publish,
    readStdin: vi.fn(async () => ''),
    write: (l) => out.stdout.push(l),
    writeErr: (l) => out.stderr.push(l),
  };
  return { deps, publish };
}

let tempDir: string;
beforeEach(() => {
  tempDir = mkdtempSync(join(tmpdir(), 'no-tickets-batch-'));
});
afterEach(() => {
  rmSync(tempDir, { recursive: true, force: true });
});

function writeBatch(content: string): string {
  const path = join(tempDir, 'batch.jsonl');
  writeFileSync(path, content);
  return path;
}

const baseOptions = (path: string): PublishBatchOptions => ({ batchPath: path });

describe('runPublishBatch — happy path', () => {
  it('reads JSONL, sends all events in a single publish call, prints summary', async () => {
    const path = writeBatch(
      '{"type": "app.user.signed-up.v1", "data": {"email": "a@b.c"}}\n' +
        '{"type": "app.user.signed-up.v1", "data": {"email": "d@e.f"}}\n',
    );
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps(
      {
        availableTypes: [TYPE],
        publishResult: { ingested: 2, deduped: 0, ids: ['e1', 'e2'] },
      },
      out,
    );

    const exit = await runPublishBatch(baseOptions(path), deps);

    expect(exit).toBe(0);
    expect(publish).toHaveBeenCalledTimes(1);
    const events = publish.mock.calls[0]?.[0];
    expect(events).toHaveLength(2);
    expect(out.stdout.join('\n')).toContain('2 event');
  });

  it('reads from stdin when --batch is "-"', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const stdin =
      '{"type": "app.user.signed-up.v1", "data": {"email": "a@b.c"}}\n';
    const deps: PublishBatchDeps = {
      listEvents: vi.fn(async () => [TYPE]),
      publish: vi.fn(async () => ({ ingested: 1, deduped: 0, ids: ['x'] })),
      readStdin: vi.fn(async () => stdin),
      write: (l) => out.stdout.push(l),
      writeErr: (l) => out.stderr.push(l),
    };

    const exit = await runPublishBatch({ batchPath: '-' }, deps);

    expect(exit).toBe(0);
    expect(deps.readStdin).toHaveBeenCalledTimes(1);
  });

  it('applies --source-name + --source-attribute to every event', async () => {
    const path = writeBatch(
      '{"type": "app.user.signed-up.v1", "data": {"email": "a@b.c"}}\n' +
        '{"type": "app.user.signed-up.v1", "data": {"email": "d@e.f"}}\n',
    );
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps(
      { availableTypes: [TYPE], publishResult: { ingested: 2, deduped: 0, ids: ['e1', 'e2'] } },
      out,
    );

    await runPublishBatch(
      {
        batchPath: path,
        sourceName: 'tiny-brain',
        sourceAttributes: ['env=prod'],
      },
      deps,
    );

    const events = publish.mock.calls[0]?.[0];
    for (const evt of events ?? []) {
      expect(evt.source).toMatchObject({
        name: 'tiny-brain',
        attributes: { env: 'prod' },
      });
    }
  });
});

describe('runPublishBatch — local validation', () => {
  it('exits with code 1 and reports the JSONL line number on parse failure', async () => {
    const path = writeBatch(
      '{"type": "app.user.signed-up.v1", "data": {"email": "a@b.c"}}\n' +
        '{not json\n',
    );
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({ availableTypes: [TYPE] }, out);

    const exit = await runPublishBatch(baseOptions(path), deps);

    expect(exit).toBe(1);
    expect(publish).not.toHaveBeenCalled();
    expect(out.stderr.join('\n')).toMatch(/line 2/);
  });

  it('exits with code 1 and reports the JSONL line number on schema-validation failure', async () => {
    const path = writeBatch(
      '{"type": "app.user.signed-up.v1", "data": {"email": "a@b.c"}}\n' +
        '{"type": "app.user.signed-up.v1", "data": {}}\n',
    );
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({ availableTypes: [TYPE] }, out);

    const exit = await runPublishBatch(baseOptions(path), deps);

    expect(exit).toBe(1);
    expect(publish).not.toHaveBeenCalled();
    const printed = out.stderr.join('\n');
    expect(printed).toMatch(/line 2/);
    expect(printed).toMatch(/email/);
  });

  it('exits with code 1 if a line is missing the type field', async () => {
    const path = writeBatch('{"data": {}}\n');
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps({ availableTypes: [TYPE] }, out);

    const exit = await runPublishBatch(baseOptions(path), deps);

    expect(exit).toBe(1);
    expect(out.stderr.join('\n')).toMatch(/line 1/);
  });

  it('exits with code 1 if any line references an unknown type id', async () => {
    const path = writeBatch(
      '{"type": "app.unknown.v1", "data": {}}\n',
    );
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps({ availableTypes: [TYPE] }, out);

    const exit = await runPublishBatch(baseOptions(path), deps);

    expect(exit).toBe(1);
    expect(out.stderr.join('\n')).toMatch(/line 1/);
    expect(out.stderr.join('\n')).toMatch(/unknown/i);
  });
});

describe('runPublishBatch — server errors map back to JSONL line numbers', () => {
  it('translates server batchIndex into the JSONL line on UnknownEventTypeError', async () => {
    const path = writeBatch(
      '{"type": "app.user.signed-up.v1", "data": {"email": "a@b.c"}}\n' +
        '{"type": "app.user.signed-up.v1", "data": {"email": "d@e.f"}}\n',
    );
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps(
      {
        availableTypes: [TYPE],
        publishError: new UnknownEventTypeError('app.user.signed-up.v1', 1),
      },
      out,
    );

    const exit = await runPublishBatch(baseOptions(path), deps);

    expect(exit).toBe(3);
    expect(out.stderr.join('\n')).toMatch(/line 2/);
  });
});
