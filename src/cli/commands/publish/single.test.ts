import { describe, it, expect, vi } from 'vitest';
import { runPublishSingle, type PublishSingleDeps, type PublishSingleOptions } from './single.js';
import type { PublishResponse, PublishEvent } from '../../../transport/events.js';

// Tests use real type ids from @magic-ingredients/no-tickets-schemas via the
// validateEventLocally path. JSON-Schema-specific validation behaviors live
// in src/cli/lib/schema-validate.test.ts now.

const VALID_EPIC_DATA = '{"epicId":"e1","projectId":"p1","title":"my epic"}';

interface RecordedOutput {
  readonly stdout: string[];
  readonly stderr: string[];
}

interface BuildDepsOptions {
  readonly publishResult?: PublishResponse;
  readonly publishError?: unknown;
  readonly stdin?: string;
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
    publish,
    readStdin: vi.fn(async () => opts.stdin ?? ''),
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
    const { deps, publish } = buildDeps({}, out);

    const exit = await runPublishSingle(
      baseOptions('product.epic.created.v1', VALID_EPIC_DATA),
      deps,
    );

    expect(exit).toBe(0);
    expect(publish).toHaveBeenCalledTimes(1);
    const events = publish.mock.calls[0]?.[0];
    expect(events).toEqual([
      expect.objectContaining({
        type: 'product.epic.created.v1',
        data: { epicId: 'e1', projectId: 'p1', title: 'my epic' },
      }),
    ]);
    expect(out.stdout.join('\n')).toContain('1 event');
    expect(out.stdout.join('\n')).toContain('evt_1');
  });

  it('attaches subject when both --subject-type and --subject-id are provided', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({}, out);

    await runPublishSingle(
      {
        ...baseOptions('product.epic.created.v1', VALID_EPIC_DATA),
        subjectType: 'app.user',
        subjectId: 'usr_42',
      },
      deps,
    );

    const event = publish.mock.calls[0]?.[0]?.[0];
    expect(event?.subject).toEqual({ type: 'app.user', id: 'usr_42' });
  });

  it('omits subject when only subject-type is provided', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({}, out);

    await runPublishSingle(
      {
        ...baseOptions('product.epic.created.v1', VALID_EPIC_DATA),
        subjectType: 'app.user',
      },
      deps,
    );

    const event = publish.mock.calls[0]?.[0]?.[0];
    expect(event?.subject).toBeUndefined();
  });

  it('omits subject when only subject-id is provided (covers the inverse half of the AND-guard)', async () => {
    // Pinned separately because buildSubject's `subjectType !== undefined`
    // and `subjectId !== undefined` checks are independent — a regression
    // dropping either half would only fail one direction.
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({}, out);

    await runPublishSingle(
      {
        ...baseOptions('product.epic.created.v1', VALID_EPIC_DATA),
        subjectId: 'usr_42',
      },
      deps,
    );

    const event = publish.mock.calls[0]?.[0]?.[0];
    expect(event?.subject).toBeUndefined();
  });

  it('attaches source overrides when source-name is provided', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({}, out);

    await runPublishSingle(
      {
        ...baseOptions('product.epic.created.v1', VALID_EPIC_DATA),
        sourceName: 'app',
        sourceAttributes: ['provider=github-actions'],
      },
      deps,
    );

    const event = publish.mock.calls[0]?.[0]?.[0];
    expect(event?.source).toEqual({
      name: 'app',
      attributes: { provider: 'github-actions' },
    });
  });

  it('defaults source to exactly { name: "cli" } on every event when no source flags supplied', async () => {
    // Surface-specific defaults replace the old CI auto-detection. The CLI
    // surface stamps `name: 'cli'` so events landed via `nt publish` are
    // distinguishable from MCP / direct-SDK provenance without the caller
    // having to pass --source-name on every invocation.
    // `toEqual` pins exact shape — a regression that smuggled `sdkVersion`
    // / `attributes` / leftover-undefined keys into the surface default
    // would fail here.
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({}, out);

    await runPublishSingle(
      baseOptions('product.epic.created.v1', VALID_EPIC_DATA),
      deps,
    );

    const event = publish.mock.calls[0]?.[0]?.[0];
    expect(event?.source).toEqual({ name: 'cli' });
  });

  it('--source-attribute attaches to the default cli source (no --source-name needed)', async () => {
    // The CLI default `name: 'cli'` stays unless --source-name is supplied;
    // --source-attribute alone should not strip the surface tag.
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({}, out);

    await runPublishSingle(
      {
        ...baseOptions('product.epic.created.v1', VALID_EPIC_DATA),
        sourceAttributes: ['provider=github-actions', 'runId=1234'],
      },
      deps,
    );

    const event = publish.mock.calls[0]?.[0]?.[0];
    expect(event?.source).toEqual({
      name: 'cli',
      attributes: { provider: 'github-actions', runId: '1234' },
    });
  });
});

describe('runPublishSingle — unknown type id', () => {
  it('exits with code 2 and prints fuzzy-match suggestions under a "Did you mean:" header (no publish call)', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({}, out);

    const exit = await runPublishSingle(
      // misspelling of product.epic.created.v1 — should fuzzy-match
      baseOptions('product.epic.creatd.v1', '{}'),
      deps,
    );

    expect(exit).toBe(2);
    expect(publish).not.toHaveBeenCalled();
    const stderr = out.stderr.join('\n');
    expect(stderr).toMatch(/Unknown event type/i);
    // Pin the literal header and the suggestion content. Without the
    // header check, mutating "Did you mean:" → "" still passes; without
    // the content check, mutating the suggestion list builder also passes.
    expect(stderr).toContain('Did you mean:');
    expect(stderr).toMatch(/product\.epic\.created\.v1/);
  });

  it('caps fuzzy-match suggestions at 3 (topN guard)', async () => {
    // The byTypeId registry has 11 entries; if topN were unbounded all 11
    // would print. Pin the upper bound so a regression to topN > 3 fails
    // here. Indented suggestion lines look like "  <id>".
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps({}, out);

    await runPublishSingle(baseOptions('a', '{}'), deps);

    const indentedLines = out.stderr.filter((l) => l.startsWith('  '));
    expect(indentedLines.length).toBeLessThanOrEqual(3);
  });

  // Note: fuzzy-match has no quality threshold — with the bundled byTypeId
  // always non-empty, "Did you mean" suggestions are always printed. The
  // `if (suggestions.length > 0)` branch in single.ts is defensive (covers
  // a future scenario where byTypeId could be empty) but unreachable in
  // production today, so there's no test for "no Did you mean" — that
  // would test dead code.
});

describe('runPublishSingle — local validation', () => {
  it('exits with code 1 and reports the count + field path when data is missing a required field', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({}, out);

    const exit = await runPublishSingle(
      // projectId missing
      baseOptions('product.epic.created.v1', '{"epicId":"e1","title":"t"}'),
      deps,
    );

    expect(exit).toBe(1);
    expect(publish).not.toHaveBeenCalled();
    const stderr = out.stderr.join('\n');
    // Pin the literal summary line — N local validation error(s):
    // Without this, a regression that drops the count or rephrases
    // "validation error(s)" → "schema problems" silently passes.
    expect(stderr).toMatch(/local validation error\(s\)/);
    // Pin that the type id is in the header — single.ts prefixes
    // "<typeId>: N local validation error(s):"
    expect(stderr).toMatch(/product\.epic\.created\.v1/);
    expect(stderr).toMatch(/projectId/);
  });

  it('exits with code 1 when a string field is empty (.min(1) violation)', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({}, out);

    const exit = await runPublishSingle(
      baseOptions(
        'product.epic.created.v1',
        '{"epicId":"","projectId":"p1","title":"t"}',
      ),
      deps,
    );

    expect(exit).toBe(1);
    expect(publish).not.toHaveBeenCalled();
    expect(out.stderr.join('\n')).toMatch(/epicId/);
  });

  it('exits with code 1 and names the extraneous key when a .strict() schema is violated', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({}, out);

    const exit = await runPublishSingle(
      baseOptions(
        'product.epic.created.v1',
        '{"epicId":"e1","projectId":"p1","title":"t","stray":1}',
      ),
      deps,
    );

    expect(exit).toBe(1);
    expect(publish).not.toHaveBeenCalled();
    // Pin that the error actually names the offending field — without this,
    // a regression that drops .strict() from the schema would still pass.
    expect(out.stderr.join('\n')).toMatch(/stray/);
  });
});

describe('runPublishSingle — operation order', () => {
  it('reports unknown_event_type (exit 2) before parsing data, even when --data is malformed JSON', async () => {
    // Regression guard: an unknown type id with bad JSON should surface the
    // type-id problem (exit 2) — the more useful signal — not be masked by
    // the JSON-parse failure (exit 1) running first.
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({}, out);

    const exit = await runPublishSingle(
      baseOptions('definitely.not.a.thing.v9', '{not-valid-json}'),
      deps,
    );

    expect(exit).toBe(2);
    expect(publish).not.toHaveBeenCalled();
    expect(out.stderr.join('\n')).toMatch(/Unknown event type/i);
  });
});

describe('runPublishSingle — optional field omission on the wire', () => {
  it('omits parentEventId, traceId, dedupeKey, and subject when not supplied (source still carries the cli surface tag)', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({}, out);

    await runPublishSingle(baseOptions('product.epic.created.v1', VALID_EPIC_DATA), deps);

    const event = publish.mock.calls[0]?.[0]?.[0];
    expect(event && 'parentEventId' in event).toBe(false);
    expect(event && 'traceId' in event).toBe(false);
    expect(event && 'dedupeKey' in event).toBe(false);
    expect(event && 'subject' in event).toBe(false);
    // source is no longer omitted — the cli surface default is unconditional.
    expect(event?.source).toEqual({ name: 'cli' });
  });

  it('passes parentEventId, traceId, and dedupeKey through when supplied', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({}, out);

    await runPublishSingle(
      {
        ...baseOptions('product.epic.created.v1', VALID_EPIC_DATA),
        parent: 'evt_parent',
        trace: 'trace_xyz',
        dedupeKey: 'dedupe_abc',
      },
      deps,
    );

    const event = publish.mock.calls[0]?.[0]?.[0];
    expect(event?.parentEventId).toBe('evt_parent');
    expect(event?.traceId).toBe('trace_xyz');
    expect(event?.dedupeKey).toBe('dedupe_abc');
  });
});

describe('runPublishSingle — input guards', () => {
  it('exits with code 1 when type id is empty (no fuzzy-match, no publish)', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({}, out);

    const exit = await runPublishSingle(baseOptions('', '{}'), deps);

    expect(exit).toBe(1);
    expect(publish).not.toHaveBeenCalled();
  });

  it('exits with code 1 when --data fails to resolve (invalid JSON)', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps({}, out);

    const exit = await runPublishSingle(
      baseOptions('product.epic.created.v1', '{not-valid-json}'),
      deps,
    );

    expect(exit).toBe(1);
    expect(publish).not.toHaveBeenCalled();
  });
});

describe('runPublishSingle — server error', () => {
  it('exits with code 3 when the publish call throws', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, publish } = buildDeps(
      { publishError: new Error('boom') },
      out,
    );

    const exit = await runPublishSingle(
      baseOptions('product.epic.created.v1', VALID_EPIC_DATA),
      deps,
    );

    expect(exit).toBe(3);
    expect(publish).toHaveBeenCalledTimes(1);
    expect(out.stderr.join('\n')).toMatch(/boom/);
  });
});
