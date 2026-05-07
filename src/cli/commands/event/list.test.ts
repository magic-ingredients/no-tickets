import { describe, it, expect, vi } from 'vitest';
import { runEventList, type EventListDeps } from './list.js';
import type { EventTypeSpec } from '../../../registry/client.js';

const TYPE_USER: EventTypeSpec = {
  id: 'app.user.signed-up.v1',
  domain: 'app.user',
  entity: 'user',
  action: 'signed-up',
  version: 1,
  schema: { type: 'object', properties: {} },
};

const TYPE_DEPLOY: EventTypeSpec = {
  id: 'engineering.deploy.completed.v1',
  domain: 'engineering',
  entity: 'deploy',
  action: 'completed',
  version: 1,
  schema: { type: 'object', properties: {} },
};

const TYPE_DEPRECATED: EventTypeSpec = {
  id: 'app.legacy.thing.v1',
  domain: 'app.legacy',
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
  it('groups types by domain', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = makeDeps([TYPE_USER, TYPE_DEPLOY], out);

    const exitCode = await runEventList({}, deps);

    expect(exitCode).toBe(0);
    const printed = out.stdout.join('\n');
    expect(printed).toContain('app.user');
    expect(printed).toContain('engineering');
    expect(printed).toContain(TYPE_USER.id);
    expect(printed).toContain(TYPE_DEPLOY.id);
    // Group headers come before their contents.
    expect(printed.indexOf('app.user')).toBeLessThan(printed.indexOf(TYPE_USER.id));
    expect(printed.indexOf('engineering')).toBeLessThan(printed.indexOf(TYPE_DEPLOY.id));
  });

  it('passes the --domain filter through to listEvents', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = makeDeps([TYPE_USER, TYPE_DEPLOY], out);

    await runEventList({ domain: 'engineering' }, deps);

    expect(deps.listEvents).toHaveBeenCalledWith({ domain: 'engineering' });
    const printed = out.stdout.join('\n');
    expect(printed).toContain(TYPE_DEPLOY.id);
    expect(printed).not.toContain(TYPE_USER.id);
  });

  it('passes --deprecated through and marks deprecated types in the output', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = makeDeps([TYPE_USER, TYPE_DEPRECATED], out);

    await runEventList({ deprecated: true }, deps);

    expect(deps.listEvents).toHaveBeenCalledWith({ deprecated: true });
    const printed = out.stdout.join('\n');
    expect(printed).toContain(TYPE_DEPRECATED.id);
    // Deprecated types are marked. Specific marker is implementation-defined,
    // but the line must include the word "deprecated" somewhere recognisable.
    const deprecatedLine = out.stdout.find((line) => line.includes(TYPE_DEPRECATED.id));
    expect(deprecatedLine).toMatch(/deprecated/i);
  });

  it('does NOT mark non-deprecated types as deprecated', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = makeDeps([TYPE_USER], out);

    await runEventList({}, deps);

    const userLine = out.stdout.find((line) => line.includes(TYPE_USER.id));
    expect(userLine).not.toMatch(/deprecated/i);
  });

  it('prints a clear empty-state message when the cache yields no types', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = makeDeps([], out);

    const exitCode = await runEventList({}, deps);

    expect(exitCode).toBe(0);
    expect(out.stdout.join('\n')).toMatch(/no.*event types/i);
  });

  it('reports a missing-cache error to stderr and exits non-zero', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const listEvents = vi.fn(async () => {
      throw new Error(
        'Registry cache not found. Populate it with `nt event list` or wait for first refresh.',
      );
    });
    const deps: EventListDeps = {
      listEvents,
      write: (line) => out.stdout.push(line),
      writeErr: (line) => out.stderr.push(line),
    };

    const exitCode = await runEventList({}, deps);

    expect(exitCode).not.toBe(0);
    expect(out.stderr.join('\n')).toMatch(/cache/i);
  });
});
