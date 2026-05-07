import { describe, it, expect, vi } from 'vitest';
import { runEventList, type EventListDeps } from './list.js';
import type { EventTypeSpec } from '../../../registry/client.js';
import { CACHE_MISSING_MESSAGE } from '../../../registry/index.js';

// Use domain names that are NOT prefixes of any type id so substring-based
// position checks can't accidentally find the domain inside the id.
const TYPE_USER: EventTypeSpec = {
  id: 'product.user.signed-up.v1',
  domain: 'people-team',
  entity: 'user',
  action: 'signed-up',
  version: 1,
  schema: { type: 'object', properties: {} },
};

const TYPE_DEPLOY: EventTypeSpec = {
  id: 'product.deploy.completed.v1',
  domain: 'platform-team',
  entity: 'deploy',
  action: 'completed',
  version: 1,
  schema: { type: 'object', properties: {} },
};

const TYPE_DEPRECATED: EventTypeSpec = {
  id: 'product.legacy.thing.v1',
  domain: 'sunset-team',
  entity: 'thing',
  action: 'event',
  version: 1,
  schema: { type: 'object', properties: {} },
  deprecatedAt: '2026-01-01T00:00:00Z',
};

interface RecordedOutput {
  readonly stdout: string[];
  readonly stderr: string[];
}

function makeDeps(types: readonly EventTypeSpec[], output: RecordedOutput): EventListDeps {
  return {
    listEvents: vi.fn(async (opts: { domain?: string; deprecated?: boolean } = {}) => {
      let filtered: readonly EventTypeSpec[] = types;
      if (opts.domain !== undefined) {
        filtered = filtered.filter((t) => t.domain === opts.domain);
      }
      if (opts.deprecated !== undefined) {
        filtered = filtered.filter((t) => {
          const dep = t.deprecatedAt !== null && t.deprecatedAt !== undefined;
          return dep === opts.deprecated;
        });
      }
      return filtered;
    }),
    write: (line: string) => output.stdout.push(line),
    writeErr: (line: string) => output.stderr.push(line),
  };
}

describe('runEventList', () => {
  it('groups types by domain — emits a header LINE for each domain followed by indented type ids', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = makeDeps([TYPE_USER, TYPE_DEPLOY], out);

    const exitCode = await runEventList({}, deps);

    expect(exitCode).toBe(0);
    // Domains are written as standalone lines (not embedded in type-id
    // strings). Check structurally rather than via substring of joined text
    // — domain names aren't prefixes of type ids in this fixture, so this
    // really proves the header line was emitted.
    expect(out.stdout).toContain('people-team');
    expect(out.stdout).toContain('platform-team');

    const userIdLine = out.stdout.findIndex((l) => l.includes(TYPE_USER.id));
    const userHeader = out.stdout.indexOf('people-team');
    expect(userHeader).toBeGreaterThanOrEqual(0);
    expect(userHeader).toBeLessThan(userIdLine);

    const deployIdLine = out.stdout.findIndex((l) => l.includes(TYPE_DEPLOY.id));
    const deployHeader = out.stdout.indexOf('platform-team');
    expect(deployHeader).toBeGreaterThanOrEqual(0);
    expect(deployHeader).toBeLessThan(deployIdLine);
  });

  it('sorts domain headers alphabetically regardless of input order', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    // Insert in non-alphabetical order: platform-team comes before people-team.
    const deps = makeDeps([TYPE_DEPLOY, TYPE_USER], out);

    await runEventList({}, deps);

    const peopleIdx = out.stdout.indexOf('people-team');
    const platformIdx = out.stdout.indexOf('platform-team');
    expect(peopleIdx).toBeGreaterThanOrEqual(0);
    expect(platformIdx).toBeGreaterThanOrEqual(0);
    expect(peopleIdx).toBeLessThan(platformIdx);
  });

  it('passes the --domain filter through to listEvents (verified by output, not call shape)', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = makeDeps([TYPE_USER, TYPE_DEPLOY], out);

    await runEventList({ domain: 'platform-team' }, deps);

    expect(deps.listEvents).toHaveBeenCalledWith(
      expect.objectContaining({ domain: 'platform-team' }),
    );
    const printed = out.stdout.join('\n');
    expect(printed).toContain(TYPE_DEPLOY.id);
    expect(printed).not.toContain(TYPE_USER.id);
  });

  it('forwards no filter keys when no options are supplied', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = makeDeps([TYPE_USER], out);

    await runEventList({}, deps);

    const callArg = vi.mocked(deps.listEvents).mock.calls[0]?.[0];
    expect(Object.keys(callArg ?? {})).toEqual([]);
  });

  it('passes --deprecated through and marks deprecated types in the output', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = makeDeps([TYPE_USER, TYPE_DEPRECATED], out);

    await runEventList({ deprecated: true }, deps);

    expect(deps.listEvents).toHaveBeenCalledWith(
      expect.objectContaining({ deprecated: true }),
    );
    const printed = out.stdout.join('\n');
    expect(printed).toContain(TYPE_DEPRECATED.id);
    const deprecatedLine = out.stdout.find((line) => line.includes(TYPE_DEPRECATED.id));
    expect(deprecatedLine).toBe(`  ${TYPE_DEPRECATED.id} (deprecated)`);
  });

  it('does NOT mark non-deprecated types as deprecated', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = makeDeps([TYPE_USER], out);

    await runEventList({}, deps);

    const userLine = out.stdout.find((line) => line.includes(TYPE_USER.id));
    // Pin the EXACT line — two-space indent, type id, no suffix. Catches
    // both "indent stripped" and "stryker-injected suffix" mutations.
    expect(userLine).toBe(`  ${TYPE_USER.id}`);
  });

  it('treats deprecatedAt: null as NOT deprecated (no marker on the line)', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const NOT_DEPRECATED_NULL: EventTypeSpec = {
      ...TYPE_USER,
      id: 'product.user.session.v1',
      deprecatedAt: null,
    };
    const deps = makeDeps([NOT_DEPRECATED_NULL], out);

    await runEventList({}, deps);

    const line = out.stdout.find((l) => l.includes(NOT_DEPRECATED_NULL.id));
    expect(line).toBe(`  ${NOT_DEPRECATED_NULL.id}`);
  });

  it('groups multiple types under the same domain into one header section', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const SECOND_USER_TYPE: EventTypeSpec = {
      ...TYPE_USER,
      id: 'product.user.deactivated.v1',
      action: 'deactivated',
    };
    const deps = makeDeps([TYPE_USER, SECOND_USER_TYPE], out);

    await runEventList({}, deps);

    // Exactly one occurrence of the domain header — proves the second type
    // pushes into the existing group rather than creating a new one.
    const headers = out.stdout.filter((l) => l === 'people-team');
    expect(headers).toHaveLength(1);
    // Both type ids appear under that header.
    const headerIdx = out.stdout.indexOf('people-team');
    const firstIdx = out.stdout.findIndex((l) => l.includes(TYPE_USER.id));
    const secondIdx = out.stdout.findIndex((l) => l.includes(SECOND_USER_TYPE.id));
    expect(firstIdx).toBeGreaterThan(headerIdx);
    expect(secondIdx).toBeGreaterThan(headerIdx);
  });

  it('prints a clear empty-state message when the cache yields no types', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = makeDeps([], out);

    const exitCode = await runEventList({}, deps);

    expect(exitCode).toBe(0);
    expect(out.stdout.join('\n')).toMatch(/no.*event types/i);
  });

  it('reports a missing-cache error to stderr verbatim and exits non-zero', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const listEvents = vi.fn(async () => {
      throw new Error(CACHE_MISSING_MESSAGE);
    });
    const deps: EventListDeps = {
      listEvents,
      write: (line) => out.stdout.push(line),
      writeErr: (line) => out.stderr.push(line),
    };

    const exitCode = await runEventList({}, deps);

    expect(exitCode).not.toBe(0);
    expect(out.stderr.join('\n')).toContain(CACHE_MISSING_MESSAGE);
  });
});
