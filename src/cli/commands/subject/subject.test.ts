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

  it('exits 1 with a clear message when metadata JSON is invalid', async () => {
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
    expect(out.stderr.join('\n')).toMatch(/metadata/i);
  });

  it('exits 1 when metadata parses as JSON but is not an object (e.g. an array)', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps({}, out);

    const exit = await runSubjectCreate(
      {
        type: 'app.user',
        externalId: 'usr_1',
        displayName: 'Ada',
        metadata: '[1, 2, 3]',
      },
      deps,
    );

    expect(exit).toBe(1);
    expect(deps.create).not.toHaveBeenCalled();
    expect(out.stderr.join('\n')).toMatch(/must be a JSON object/i);
  });

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

  it('renders a table with one line per subject when --format table is requested', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps(
      {
        listResult: [
          { ...SAMPLE, externalId: 'usr_1' },
          { ...SAMPLE, externalId: 'usr_2', displayName: 'Bob' },
        ],
      },
      out,
    );

    await runSubjectList({ type: 'app.user', format: 'table' }, deps);

    // First line is the header.
    expect(out.stdout[0]?.toLowerCase()).toMatch(/external/);
    // Each subject occupies its own line; assert the externalId is on a
    // single line that also carries the displayName for that subject.
    const ada = out.stdout.find((l) => l.includes('usr_1'));
    expect(ada).toBeDefined();
    expect(ada).toContain('Ada');
    const bob = out.stdout.find((l) => l.includes('usr_2'));
    expect(bob).toBeDefined();
    expect(bob).toContain('Bob');
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

  it('exits 1 with a "--type is required" message when the type filter is empty', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = buildDeps({}, out);

    const exit = await runSubjectList({ type: '' }, deps);

    expect(exit).toBe(1);
    expect(deps.list).not.toHaveBeenCalled();
    expect(out.stderr.join('\n')).toMatch(/--type is required/);
  });
});
