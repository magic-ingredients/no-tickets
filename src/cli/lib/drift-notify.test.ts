import { describe, it, expect, vi, afterEach } from 'vitest';
import { notifyDrift, type DriftNotifyDeps } from './drift-notify.js';
import type { CacheFile } from '../../registry/cache.js';
import type { EventTypeSpec } from '../../registry/client.js';
import type { AwaitRefreshResult } from '../../registry/refresh.js';

const TYPE = (id: string): EventTypeSpec => ({
  id,
  domain: id.split('.')[0] ?? 'app',
  entity: 'thing',
  action: 'happened',
  version: 'v1',
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

interface BuildOpts {
  readonly priorCache: CacheFile | null;
  readonly postCache?: CacheFile | null;
  readonly priorThrow?: unknown;
  readonly postThrow?: unknown;
}

function buildDeps(opts: BuildOpts): { deps: DriftNotifyDeps; out: RecordedOutput } {
  const out: RecordedOutput = { stderr: [] };
  const deps: DriftNotifyDeps = {
    readPriorCache: vi.fn(() => {
      if (opts.priorThrow !== undefined) throw opts.priorThrow;
      return opts.priorCache;
    }),
    readPostCache: vi.fn(() => {
      if (opts.postThrow !== undefined) throw opts.postThrow;
      return opts.postCache ?? null;
    }),
    writeErr: (l) => out.stderr.push(l),
  };
  return { deps, out };
}

const refresh = (result: AwaitRefreshResult): Promise<AwaitRefreshResult> =>
  Promise.resolve(result);

describe('notifyDrift — early returns', () => {
  it('writes nothing when refresh times out', async () => {
    const { deps, out } = buildDeps({
      priorCache: CACHE([TYPE('a.b.v1')]),
      postCache: CACHE([TYPE('a.b.v1'), TYPE('a.c.v1')]),
    });

    await notifyDrift(refresh({ status: 'timeout' }), {}, deps);

    expect(out.stderr).toEqual([]);
    // readPostCache must NOT be called when refresh did not finish.
    expect(deps.readPostCache).not.toHaveBeenCalled();
  });

  it('writes nothing when refresh is unchanged', async () => {
    const { deps, out } = buildDeps({
      priorCache: CACHE([TYPE('a.b.v1')]),
      postCache: CACHE([TYPE('a.b.v1'), TYPE('a.c.v1')]),
    });

    await notifyDrift(refresh({ status: 'unchanged', etag: 'W/"x"' }), {}, deps);

    expect(out.stderr).toEqual([]);
    expect(deps.readPostCache).not.toHaveBeenCalled();
  });

  it('writes nothing when refresh fails', async () => {
    const { deps, out } = buildDeps({
      priorCache: CACHE([TYPE('a.b.v1')]),
      postCache: CACHE([TYPE('a.b.v1'), TYPE('a.c.v1')]),
    });

    await notifyDrift(
      refresh({ status: 'failed', error: new Error('offline') }),
      {},
      deps,
    );

    expect(out.stderr).toEqual([]);
    expect(deps.readPostCache).not.toHaveBeenCalled();
  });

  it('writes nothing when refresh updates but the type set is identical', async () => {
    const types = [TYPE('a.b.v1'), TYPE('a.c.v1')];
    const { deps, out } = buildDeps({
      priorCache: CACHE(types),
      postCache: CACHE(types),
    });

    await notifyDrift(refresh({ status: 'updated', etag: 'W/"new"' }), {}, deps);

    expect(out.stderr).toEqual([]);
  });

  it('writes nothing when the prior cache is missing — no baseline', async () => {
    const { deps, out } = buildDeps({
      priorCache: null,
      postCache: CACHE([TYPE('a.b.v1')]),
    });

    await notifyDrift(refresh({ status: 'updated', etag: 'W/"new"' }), {}, deps);

    expect(out.stderr).toEqual([]);
    // We bail before even awaiting the refresh promise's effect on
    // readPostCache.
    expect(deps.readPostCache).not.toHaveBeenCalled();
  });

  it('writes nothing when the post-refresh cache disappears (read returns null)', async () => {
    const { deps, out } = buildDeps({
      priorCache: CACHE([TYPE('a.b.v1')]),
      postCache: null,
    });

    await notifyDrift(refresh({ status: 'updated', etag: 'W/"new"' }), {}, deps);

    expect(out.stderr).toEqual([]);
  });
});

describe('notifyDrift — formatted output', () => {
  it('lists ALL new ids when 1-3 of them, with the exact "since last sync" phrasing and ℹ glyph', async () => {
    const { deps, out } = buildDeps({
      priorCache: CACHE([TYPE('a.b.v1')]),
      postCache: CACHE([TYPE('a.b.v1'), TYPE('a.c.v1'), TYPE('engineering.deploy.v1')]),
    });

    await notifyDrift(refresh({ status: 'updated', etag: 'W/"new"' }), {}, deps);

    expect(out.stderr).toHaveLength(1);
    expect(out.stderr[0]).toBe(
      'ℹ 2 new event types since last sync: a.c.v1, engineering.deploy.v1',
    );
  });

  it('lists exactly 3 new ids WITHOUT the ", ..." suffix when count == MAX_LISTED', async () => {
    // Boundary test pinning `length > MAX_LISTED` (not `>=`). At exactly 3,
    // every id fits and the truncation marker must NOT appear.
    const { deps, out } = buildDeps({
      priorCache: CACHE([]),
      postCache: CACHE([TYPE('a.1.v1'), TYPE('a.2.v1'), TYPE('a.3.v1')]),
    });

    await notifyDrift(refresh({ status: 'updated', etag: 'W/"new"' }), {}, deps);

    expect(out.stderr).toEqual([
      'ℹ 3 new event types since last sync: a.1.v1, a.2.v1, a.3.v1',
    ]);
  });

  it('lists exactly the first 3 new ids and appends ", ..." when more than 3', async () => {
    const { deps, out } = buildDeps({
      priorCache: CACHE([]),
      postCache: CACHE([
        TYPE('a.1.v1'),
        TYPE('a.2.v1'),
        TYPE('a.3.v1'),
        TYPE('a.4.v1'),
        TYPE('a.5.v1'),
      ]),
    });

    await notifyDrift(refresh({ status: 'updated', etag: 'W/"new"' }), {}, deps);

    expect(out.stderr).toHaveLength(1);
    expect(out.stderr[0]).toBe(
      'ℹ 5 new event types since last sync: a.1.v1, a.2.v1, a.3.v1, ...',
    );
  });
});

describe('notifyDrift — quiet flag', () => {
  it('is suppressed by --quiet (no cache reads at all)', async () => {
    const { deps, out } = buildDeps({
      priorCache: CACHE([]),
      postCache: CACHE([TYPE('a.new.v1')]),
    });

    await notifyDrift(refresh({ status: 'updated', etag: 'W/"new"' }), { quiet: true }, deps);

    expect(out.stderr).toEqual([]);
    expect(deps.readPriorCache).not.toHaveBeenCalled();
    expect(deps.readPostCache).not.toHaveBeenCalled();
  });

  it('is suppressed by NO_TICKETS_QUIET=1 in the environment', async () => {
    process.env['NO_TICKETS_QUIET'] = '1';
    const { deps, out } = buildDeps({
      priorCache: CACHE([]),
      postCache: CACHE([TYPE('a.new.v1')]),
    });

    await notifyDrift(refresh({ status: 'updated', etag: 'W/"new"' }), {}, deps);

    expect(out.stderr).toEqual([]);
    expect(deps.readPriorCache).not.toHaveBeenCalled();
  });
});

describe('notifyDrift — robustness', () => {
  it('does not throw when readPriorCache throws (treats it as no baseline)', async () => {
    const { deps, out } = buildDeps({
      priorCache: null,
      priorThrow: new Error('disk corruption'),
    });

    await expect(
      notifyDrift(refresh({ status: 'updated', etag: 'W/"new"' }), {}, deps),
    ).resolves.toBeUndefined();
    expect(out.stderr).toEqual([]);
  });

  it('does not throw when readPostCache throws (no notification, no propagation)', async () => {
    const { deps, out } = buildDeps({
      priorCache: CACHE([TYPE('a.b.v1')]),
      postThrow: new Error('disk corruption'),
    });

    await expect(
      notifyDrift(refresh({ status: 'updated', etag: 'W/"new"' }), {}, deps),
    ).resolves.toBeUndefined();
    expect(out.stderr).toEqual([]);
  });

  it('does not throw when the refresh promise itself rejects', async () => {
    const { deps, out } = buildDeps({
      priorCache: CACHE([TYPE('a.b.v1')]),
      postCache: CACHE([TYPE('a.b.v1'), TYPE('a.c.v1')]),
    });

    await expect(
      notifyDrift(Promise.reject(new Error('refresh blew up')), {}, deps),
    ).resolves.toBeUndefined();
    expect(out.stderr).toEqual([]);
  });
});
