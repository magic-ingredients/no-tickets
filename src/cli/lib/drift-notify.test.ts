import { describe, it, expect, vi, afterEach } from 'vitest';
import { notifyDrift, type DriftNotifyDeps } from './drift-notify.js';
import type { CacheFile } from '../../registry/cache.js';
import type { EventTypeSpec } from '../../registry/client.js';
import type { RefreshResult } from '../../registry/refresh.js';

const TYPE = (id: string): EventTypeSpec => ({
  id,
  domain: id.split('.')[0] ?? 'app',
  entity: 'thing',
  action: 'happened',
  version: 1,
  schema: { type: 'object', properties: {} },
});

const CACHE = (types: readonly EventTypeSpec[]): CacheFile => ({
  version: 1,
  etag: 'W/"x"',
  fetchedAt: '2026-05-07T00:00:00Z',
  serverUrl: 'https://api.example.com',
  types: [...types],
});

afterEach(() => {
  delete process.env['NO_TICKETS_QUIET'];
});

interface RecordedOutput {
  readonly stderr: string[];
}

function buildDeps(opts: {
  readonly priorCache: CacheFile | null;
  readonly refresh: RefreshResult | { status: 'timeout' };
}): { deps: DriftNotifyDeps; out: RecordedOutput } {
  const out: RecordedOutput = { stderr: [] };
  const deps: DriftNotifyDeps = {
    readPriorCache: vi.fn(() => opts.priorCache),
    awaitRefresh: vi.fn(async () => opts.refresh),
    readPostCache: vi.fn(() => null),
    writeErr: (l) => out.stderr.push(l),
  };
  return { deps, out };
}

describe('notifyDrift', () => {
  it('writes nothing when refresh times out (no diff this invocation)', async () => {
    const { deps, out } = buildDeps({
      priorCache: CACHE([TYPE('app.user.v1')]),
      refresh: { status: 'timeout' },
    });

    await notifyDrift(Promise.resolve({ status: 'timeout' }), {}, deps);

    expect(out.stderr).toEqual([]);
  });

  it('writes nothing when refresh is unchanged (no diff)', async () => {
    const { deps, out } = buildDeps({
      priorCache: CACHE([TYPE('app.user.v1')]),
      refresh: { status: 'unchanged', etag: 'W/"x"' },
    });

    await notifyDrift(
      Promise.resolve({ status: 'unchanged', etag: 'W/"x"' }),
      {},
      deps,
    );

    expect(out.stderr).toEqual([]);
  });

  it('writes nothing when refresh updates but the type set is identical', async () => {
    const types = [TYPE('a.b.v1'), TYPE('a.c.v1')];
    const deps: DriftNotifyDeps = {
      readPriorCache: vi.fn(() => CACHE(types)),
      awaitRefresh: vi.fn(),
      readPostCache: vi.fn(() => CACHE(types)),
      writeErr: vi.fn(),
    };

    await notifyDrift(
      Promise.resolve({ status: 'updated', etag: 'W/"new"' }),
      {},
      deps,
    );

    expect(deps.writeErr).not.toHaveBeenCalled();
  });

  it('lists ALL new type ids when there are 1-3 of them', async () => {
    const prior = CACHE([TYPE('a.b.v1')]);
    const next = CACHE([
      TYPE('a.b.v1'),
      TYPE('a.c.v1'),
      TYPE('engineering.deploy.v1'),
    ]);
    const out: RecordedOutput = { stderr: [] };
    const deps: DriftNotifyDeps = {
      readPriorCache: vi.fn(() => prior),
      awaitRefresh: vi.fn(),
      readPostCache: vi.fn(() => next),
      writeErr: (l) => out.stderr.push(l),
    };

    await notifyDrift(
      Promise.resolve({ status: 'updated', etag: 'W/"new"' }),
      {},
      deps,
    );

    expect(out.stderr).toHaveLength(1);
    const line = out.stderr[0] ?? '';
    expect(line).toContain('2 new event types');
    expect(line).toContain('a.c.v1');
    expect(line).toContain('engineering.deploy.v1');
    expect(line).not.toContain('...');
  });

  it('truncates with "..." when there are more than 3 new types', async () => {
    const prior = CACHE([]);
    const next = CACHE([
      TYPE('a.1.v1'),
      TYPE('a.2.v1'),
      TYPE('a.3.v1'),
      TYPE('a.4.v1'),
      TYPE('a.5.v1'),
    ]);
    const out: RecordedOutput = { stderr: [] };
    const deps: DriftNotifyDeps = {
      readPriorCache: vi.fn(() => prior),
      awaitRefresh: vi.fn(),
      readPostCache: vi.fn(() => next),
      writeErr: (l) => out.stderr.push(l),
    };

    await notifyDrift(
      Promise.resolve({ status: 'updated', etag: 'W/"new"' }),
      {},
      deps,
    );

    expect(out.stderr).toHaveLength(1);
    const line = out.stderr[0] ?? '';
    expect(line).toContain('5 new event types');
    expect(line).toContain('a.1.v1');
    expect(line).toContain('a.2.v1');
    expect(line).toContain('a.3.v1');
    expect(line).toContain('...');
    expect(line).not.toContain('a.4.v1');
    expect(line).not.toContain('a.5.v1');
  });

  it('is suppressed by --quiet', async () => {
    const prior = CACHE([]);
    const next = CACHE([TYPE('a.new.v1')]);
    const out: RecordedOutput = { stderr: [] };
    const deps: DriftNotifyDeps = {
      readPriorCache: vi.fn(() => prior),
      awaitRefresh: vi.fn(),
      readPostCache: vi.fn(() => next),
      writeErr: (l) => out.stderr.push(l),
    };

    await notifyDrift(
      Promise.resolve({ status: 'updated', etag: 'W/"new"' }),
      { quiet: true },
      deps,
    );

    expect(out.stderr).toEqual([]);
  });

  it('is suppressed by NO_TICKETS_QUIET=1 in the environment', async () => {
    process.env['NO_TICKETS_QUIET'] = '1';
    const prior = CACHE([]);
    const next = CACHE([TYPE('a.new.v1')]);
    const out: RecordedOutput = { stderr: [] };
    const deps: DriftNotifyDeps = {
      readPriorCache: vi.fn(() => prior),
      awaitRefresh: vi.fn(),
      readPostCache: vi.fn(() => next),
      writeErr: (l) => out.stderr.push(l),
    };

    await notifyDrift(
      Promise.resolve({ status: 'updated', etag: 'W/"new"' }),
      {},
      deps,
    );

    expect(out.stderr).toEqual([]);
  });

  it('writes nothing when the prior cache is missing — no baseline to diff against', async () => {
    const out: RecordedOutput = { stderr: [] };
    const deps: DriftNotifyDeps = {
      readPriorCache: vi.fn(() => null),
      awaitRefresh: vi.fn(),
      readPostCache: vi.fn(() => CACHE([TYPE('a.b.v1')])),
      writeErr: (l) => out.stderr.push(l),
    };

    await notifyDrift(
      Promise.resolve({ status: 'updated', etag: 'W/"new"' }),
      {},
      deps,
    );

    expect(out.stderr).toEqual([]);
  });
});
