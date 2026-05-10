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
import {
  UnknownEventTypeError,
  EventValidationError,
} from '../../../transport/errors.js';

const TYPE: EventTypeSpec = {
  id: 'app.user.signed-up.v1',
  domain: 'app.user',
  entity: 'user',
  action: 'signed-up',
  version: 'v1',
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
    expect(events).toEqual([
      expect.objectContaining({
        type: 'app.user.signed-up.v1',
        data: { email: 'a@b.c' },
      }),
      expect.objectContaining({
        type: 'app.user.signed-up.v1',
        data: { email: 'd@e.f' },
      }),
    ]);
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

  it('translates server batchIndex into the JSONL line on EventValidationError', async () => {
    const path = writeBatch(
      '{"type": "app.user.signed-up.v1", "data": {"email": "a@b.c"}}\n' +
        '{"type": "app.user.signed-up.v1", "data": {"email": "d@e.f"}}\n',
    );
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps(
      {
        availableTypes: [TYPE],
        publishError: new EventValidationError(
          'app.user.signed-up.v1',
          0,
          [{ path: ['data', 'email'], message: 'rejected' }],
        ),
      },
      out,
    );

    const exit = await runPublishBatch(baseOptions(path), deps);

    expect(exit).toBe(3);
    expect(out.stderr.join('\n')).toMatch(/line 1/);
  });

  it('reports JSONL lines correctly when blank lines shift the line/index alignment', async () => {
    // Blank lines between events mean batchIndex 1 corresponds to line 4,
    // not line 2. Validates that the implementation maps via the recorded
    // `line` rather than `batchIndex + 1`.
    const path = writeBatch(
      '\n' +
        '{"type": "app.user.signed-up.v1", "data": {"email": "a@b.c"}}\n' +
        '\n' +
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
    expect(out.stderr.join('\n')).toMatch(/line 4/);
  });
});

describe('runPublishBatch — non-object JSONL entries', () => {
  it('rejects a JSONL line whose value is null', async () => {
    const path = writeBatch('null\n');
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({ availableTypes: [TYPE] }, out);

    const exit = await runPublishBatch(baseOptions(path), deps);

    expect(exit).toBe(1);
    expect(publish).not.toHaveBeenCalled();
    expect(out.stderr.join('\n')).toMatch(/line 1/);
    expect(out.stderr.join('\n')).toMatch(/expected an object/);
  });

  it('rejects a JSONL line whose value is an array', async () => {
    const path = writeBatch('[1, 2, 3]\n');
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps({ availableTypes: [TYPE] }, out);

    const exit = await runPublishBatch(baseOptions(path), deps);

    expect(exit).toBe(1);
    expect(out.stderr.join('\n')).toMatch(/expected an object/);
  });

  it('rejects a JSONL line whose value is a primitive', async () => {
    const path = writeBatch('42\n');
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps({ availableTypes: [TYPE] }, out);

    const exit = await runPublishBatch(baseOptions(path), deps);

    expect(exit).toBe(1);
    expect(out.stderr.join('\n')).toMatch(/expected an object/);
  });

  it('rejects a JSONL line where type is the empty string', async () => {
    const path = writeBatch('{"type": "", "data": {}}\n');
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps({ availableTypes: [TYPE] }, out);

    const exit = await runPublishBatch(baseOptions(path), deps);

    expect(exit).toBe(1);
    expect(out.stderr.join('\n')).toMatch(/missing or empty/);
  });
});

describe('runPublishBatch — server error fallthrough', () => {
  it('handles a plain Error from publish() (exit 3, message on stderr)', async () => {
    const path = writeBatch(
      '{"type": "app.user.signed-up.v1", "data": {"email": "a@b.c"}}\n',
    );
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps(
      { availableTypes: [TYPE], publishError: new Error('plain server boom') },
      out,
    );

    const exit = await runPublishBatch(baseOptions(path), deps);

    expect(exit).toBe(3);
    expect(out.stderr.join('\n')).toContain('plain server boom');
  });

  it('falls back to "batch index N" when batchIndex is out of bounds for the local list', async () => {
    const path = writeBatch(
      '{"type": "app.user.signed-up.v1", "data": {"email": "a@b.c"}}\n',
    );
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps(
      {
        availableTypes: [TYPE],
        publishError: new UnknownEventTypeError('app.user.signed-up.v1', 99),
      },
      out,
    );

    const exit = await runPublishBatch(baseOptions(path), deps);

    expect(exit).toBe(3);
    expect(out.stderr.join('\n')).toContain('batch index 99');
  });
});

describe('runPublishBatch — source merge edge cases', () => {
  it('defaults source.name to "cli" on every event when neither JSONL nor CLI supplies one', async () => {
    // Surface-specific defaults replace the old CI auto-detection at the
    // transport layer. The batch path now stamps `name: 'cli'` for the
    // same reason single.ts does — events landed via `nt publish --batch`
    // are distinguishable from MCP / direct-SDK provenance without the
    // caller pinning --source-name on every invocation.
    const path = writeBatch(
      '{"type": "app.user.signed-up.v1", "data": {"email": "a@b.c"}}\n',
    );
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps(
      { availableTypes: [TYPE], publishResult: { ingested: 1, deduped: 0, ids: ['x'] } },
      out,
    );

    await runPublishBatch(baseOptions(path), deps);

    const event = publish.mock.calls[0]?.[0]?.[0];
    expect(event?.source).toEqual({ name: 'cli' });
  });

  it('uses CLI source verbatim when JSONL line has no source', async () => {
    const path = writeBatch(
      '{"type": "app.user.signed-up.v1", "data": {"email": "a@b.c"}}\n',
    );
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps(
      { availableTypes: [TYPE], publishResult: { ingested: 1, deduped: 0, ids: ['x'] } },
      out,
    );

    await runPublishBatch(
      { batchPath: path, sourceName: 'cli-tool' },
      deps,
    );

    const event = publish.mock.calls[0]?.[0]?.[0];
    expect(event?.source).toEqual({ name: 'cli-tool' });
  });

  it('uses JSONL source verbatim when no CLI flags supplied', async () => {
    const path = writeBatch(
      '{"type": "app.user.signed-up.v1", "data": {"email": "a@b.c"}, "source": {"name": "wrapper"}}\n',
    );
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps(
      { availableTypes: [TYPE], publishResult: { ingested: 1, deduped: 0, ids: ['x'] } },
      out,
    );

    await runPublishBatch(baseOptions(path), deps);

    const event = publish.mock.calls[0]?.[0]?.[0];
    expect(event?.source).toEqual({ name: 'wrapper' });
  });

  it('keeps CLI attributes when JSONL source has no attributes bag', async () => {
    const path = writeBatch(
      '{"type": "app.user.signed-up.v1", "data": {"email": "a@b.c"}, "source": {"name": "wrapper"}}\n',
    );
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps(
      { availableTypes: [TYPE], publishResult: { ingested: 1, deduped: 0, ids: ['x'] } },
      out,
    );

    await runPublishBatch(
      {
        batchPath: path,
        sourceAttributes: ['env=prod'],
      },
      deps,
    );

    const event = publish.mock.calls[0]?.[0]?.[0];
    expect(event?.source).toEqual({
      name: 'wrapper',
      attributes: { env: 'prod' },
    });
  });

  it('keeps JSONL attributes when CLI has no flags — surface default "cli" still wins', async () => {
    // No --source-name is passed; the cli surface default tag must still
    // appear. The JSONL line carries only `attributes.region`, no top-level
    // `name`, so mergeSourceShallow falls back to the cli default for name
    // and merges the attributes bag in.
    const path = writeBatch(
      '{"type": "app.user.signed-up.v1", "data": {"email": "a@b.c"}, "source": {"attributes": {"region": "eu"}}}\n',
    );
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps(
      { availableTypes: [TYPE], publishResult: { ingested: 1, deduped: 0, ids: ['x'] } },
      out,
    );

    await runPublishBatch(baseOptions(path), deps);

    const event = publish.mock.calls[0]?.[0]?.[0];
    expect(event?.source).toEqual({
      name: 'cli',
      attributes: { region: 'eu' },
    });
  });
});

describe('runPublishBatch — output', () => {
  it('writes one indented line per returned event id', async () => {
    const path = writeBatch(
      '{"type": "app.user.signed-up.v1", "data": {"email": "a@b.c"}}\n' +
        '{"type": "app.user.signed-up.v1", "data": {"email": "d@e.f"}}\n',
    );
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps(
      { availableTypes: [TYPE], publishResult: { ingested: 2, deduped: 0, ids: ['e1', 'e2'] } },
      out,
    );

    await runPublishBatch(baseOptions(path), deps);

    expect(out.stdout).toContain('  e1');
    expect(out.stdout).toContain('  e2');
  });
});

describe('runPublishBatch — empty file', () => {
  it('exits with code 1 and reports an empty-file message on stderr', async () => {
    const path = writeBatch('');
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({ availableTypes: [TYPE] }, out);

    const exit = await runPublishBatch(baseOptions(path), deps);

    expect(exit).toBe(1);
    expect(publish).not.toHaveBeenCalled();
    expect(out.stderr.join('\n')).toMatch(/empty/i);
  });
});

describe('runPublishBatch — source merge', () => {
  it('merges CLI --source-attribute keys with JSONL source.attributes (JSONL wins on key conflict)', async () => {
    const path = writeBatch(
      '{"type": "app.user.signed-up.v1", "data": {"email": "a@b.c"}, "source": {"attributes": {"region": "eu-west-1"}}}\n',
    );
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps(
      { availableTypes: [TYPE], publishResult: { ingested: 1, deduped: 0, ids: ['x'] } },
      out,
    );

    await runPublishBatch(
      {
        batchPath: path,
        sourceName: 'tiny-brain',
        sourceAttributes: ['env=prod', 'region=us-east-1'],
      },
      deps,
    );

    const event = publish.mock.calls[0]?.[0]?.[0];
    // CLI brings env=prod and region=us-east-1; JSONL overrides region.
    expect(event?.source).toMatchObject({
      name: 'tiny-brain',
      attributes: { env: 'prod', region: 'eu-west-1' },
    });
  });
});
