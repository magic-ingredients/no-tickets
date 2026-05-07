import { describe, it, expect, vi } from 'vitest';
import { runSubjectCreate, runSubjectGet, runSubjectList, type SubjectDeps } from './subject.js';
import type { Subject, SubjectRef } from '../../../core/subject.js';
import { HttpError } from '../../../transport/errors.js';

const SAMPLE: Subject = {
  type: 'app.user',
  externalId: 'usr_1',
  displayName: 'Ada',
  metadata: { plan: 'pro' },
};

interface RecordedOutput {
  readonly stdout: string[];
  readonly stderr: string[];
}

interface BuildDepsOptions {
  readonly createResult?: Subject;
  readonly getResult?: Subject;
  readonly listResult?: readonly Subject[];
  readonly error?: unknown;
}

function buildDeps(opts: BuildDepsOptions, out: RecordedOutput): SubjectDeps {
  return {
    create: vi.fn(async () => {
      if (opts.error !== undefined) throw opts.error;
      return opts.createResult ?? SAMPLE;
    }),
    get: vi.fn(async (_ref: SubjectRef) => {
      if (opts.error !== undefined) throw opts.error;
      return opts.getResult ?? SAMPLE;
    }),
    list: vi.fn(async () => {
      if (opts.error !== undefined) throw opts.error;
      return opts.listResult ?? [SAMPLE];
    }),
    write: (l) => out.stdout.push(l),
    writeErr: (l) => out.stderr.push(l),
  };
}

describe('runSubjectCreate', () => {
  it('forwards the constructed Subject to deps.create and prints JSON output', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps({}, out);

    const exit = await runSubjectCreate(
      {
        type: 'app.user',
        externalId: 'usr_1',
        displayName: 'Ada',
        metadata: '{"plan": "pro"}',
      },
      deps,
    );

    expect(exit).toBe(0);
    expect(deps.create).toHaveBeenCalledWith(SAMPLE);
    expect(out.stdout.join('\n')).toContain('"externalId": "usr_1"');
  });

  it('omits metadata from the wire when not supplied', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps({}, out);

    await runSubjectCreate(
      {
        type: 'app.user',
        externalId: 'usr_1',
        displayName: 'Ada',
      },
      deps,
    );

    const arg = vi.mocked(deps.create).mock.calls[0]?.[0];
    expect(arg).not.toHaveProperty('metadata');
  });

  it('exits 1 with a clear message when metadata JSON is invalid (and writeErr is called)', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps({}, out);

    const exit = await runSubjectCreate(
      {
        type: 'app.user',
        externalId: 'usr_1',
        displayName: 'Ada',
        metadata: '{not json',
      },
      deps,
    );

    expect(exit).toBe(1);
    expect(deps.create).not.toHaveBeenCalled();
    expect(out.stderr.length).toBeGreaterThan(0);
    expect(out.stderr.join('\n')).toMatch(/metadata/i);
  });

  it.each([
    ['array', '[1, 2, 3]'],
    ['null', 'null'],
    ['number', '42'],
    ['string', '"a string"'],
  ])(
    'exits 1 when metadata parses as JSON but is a %s (not an object)',
    async (_kind, metadata) => {
      const out: RecordedOutput = { stdout: [], stderr: [] };
      const deps = buildDeps({}, out);

      const exit = await runSubjectCreate(
        {
          type: 'app.user',
          externalId: 'usr_1',
          displayName: 'Ada',
          metadata,
        },
        deps,
      );

      expect(exit).toBe(1);
      expect(deps.create).not.toHaveBeenCalled();
      expect(out.stderr.join('\n')).toMatch(/must be a JSON object/i);
    },
  );

  it('prints the created Subject as PRETTY-PRINTED JSON (2-space indent)', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps({}, out);

    await runSubjectCreate(
      { type: 'app.user', externalId: 'usr_1', displayName: 'Ada' },
      deps,
    );

    expect(out.stdout.join('\n')).toContain(JSON.stringify(SAMPLE, null, 2));
  });

  it('exits 3 on a server error', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps({ error: new Error('boom') }, out);

    const exit = await runSubjectCreate(
      { type: 'app.user', externalId: 'usr_1', displayName: 'Ada' },
      deps,
    );

    expect(exit).toBe(3);
    expect(out.stderr.join('\n')).toContain('boom');
  });
});

describe('runSubjectGet', () => {
  it('forwards the SubjectRef to deps.get and prints the result as JSON', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps({}, out);

    const exit = await runSubjectGet({ type: 'app.user', id: 'usr_1' }, deps);

    expect(exit).toBe(0);
    expect(deps.get).toHaveBeenCalledWith({ type: 'app.user', id: 'usr_1' });
    expect(out.stdout.join('\n')).toContain('"externalId": "usr_1"');
  });

  it('exits 1 with a "<type>/<id>" formatted message on HttpError 404', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps(
      { error: new HttpError(404, { msg: 'not found' }) },
      out,
    );

    const exit = await runSubjectGet({ type: 'app.user', id: 'usr_missing' }, deps);

    expect(exit).toBe(1);
    expect(out.stderr.join('\n')).toContain('app.user/usr_missing');
  });

  it('exits 1 without calling deps.get when type is empty', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps({}, out);

    const exit = await runSubjectGet({ type: '', id: 'usr_1' }, deps);

    expect(exit).toBe(1);
    expect(deps.get).not.toHaveBeenCalled();
    expect(out.stderr.join('\n')).toMatch(/--type/);
  });

  it('exits 1 without calling deps.get when id is empty', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps({}, out);

    const exit = await runSubjectGet({ type: 'app.user', id: '' }, deps);

    expect(exit).toBe(1);
    expect(deps.get).not.toHaveBeenCalled();
    expect(out.stderr.join('\n')).toMatch(/--id/);
  });

  it('exits 3 on a non-404 server error', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps(
      { error: new HttpError(500, { msg: 'internal' }) },
      out,
    );

    const exit = await runSubjectGet({ type: 'app.user', id: 'usr_1' }, deps);

    expect(exit).toBe(3);
  });
});

describe('runSubjectList', () => {
  it('forwards the type filter to deps.list and prints a JSON array by default', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps({}, out);

    const exit = await runSubjectList({ type: 'app.user' }, deps);

    expect(exit).toBe(0);
    expect(deps.list).toHaveBeenCalledWith({ type: 'app.user' });
    expect(out.stdout.join('\n')).toContain('"externalId": "usr_1"');
  });

  it('renders a table with externalId + displayName columns separated by 2+ spaces', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps(
      {
        // Asymmetric column lengths so the Math.max-based padding has to
        // pad usr_1 out to match the longer usr_lengthy_id width.
        listResult: [
          { ...SAMPLE, externalId: 'usr_1', displayName: 'Ada' },
          { ...SAMPLE, externalId: 'usr_lengthy_id', displayName: 'Bob' },
        ],
      },
      out,
    );

    await runSubjectList({ type: 'app.user', format: 'table' }, deps);

    // Header has BOTH column names — pins the StringLiteral mutations.
    expect(out.stdout[0]).toContain('externalId');
    expect(out.stdout[0]).toContain('displayName');

    // Each subject's externalId is on its own line, paired with two-space
    // (or more) column separation followed by its displayName.
    const ada = out.stdout.find((l) => l.includes('usr_1'));
    expect(ada).toMatch(/usr_1\s{2,}Ada/);
    const bob = out.stdout.find((l) => l.includes('usr_lengthy_id'));
    expect(bob).toMatch(/usr_lengthy_id\s{2,}Bob/);
    // Padding works: the short externalId line must have at least as many
    // chars before "Ada" as the long one has before "Bob".
    const adaPadding = (ada ?? '').indexOf('Ada');
    const bobPadding = (bob ?? '').indexOf('Bob');
    expect(adaPadding).toBeGreaterThanOrEqual(bobPadding);
  });

  it('uses pretty-printed JSON for the default format', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps({ listResult: [SAMPLE] }, out);

    await runSubjectList({ type: 'app.user' }, deps);

    expect(out.stdout.join('\n')).toBe(JSON.stringify([SAMPLE], null, 2));
  });

  it('uses pretty-printed JSON when --format is explicitly "json"', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps({ listResult: [SAMPLE] }, out);

    await runSubjectList({ type: 'app.user', format: 'json' }, deps);

    expect(out.stdout.join('\n')).toBe(JSON.stringify([SAMPLE], null, 2));
  });

  it('handles an empty result with a recognisable empty-state message', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps({ listResult: [] }, out);

    await runSubjectList({ type: 'app.user', format: 'table' }, deps);

    expect(out.stdout.join('\n')).toMatch(/no subjects/i);
  });

  it('exits 3 on a server error from deps.list', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps({ error: new Error('service unavailable') }, out);

    const exit = await runSubjectList({ type: 'app.user' }, deps);

    expect(exit).toBe(3);
    expect(out.stderr.join('\n')).toContain('service unavailable');
  });

  it('exits 1 with a "--type is required" message when the type filter is empty', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps({}, out);

    const exit = await runSubjectList({ type: '' }, deps);

    expect(exit).toBe(1);
    expect(deps.list).not.toHaveBeenCalled();
    expect(out.stderr.join('\n')).toMatch(/--type is required/);
  });
});
