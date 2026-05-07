import { describe, it, expect, vi } from 'vitest';
import { runEventDescribe, type EventDescribeDeps } from './describe.js';
import type { EventTypeSpec } from '../../../registry/client.js';

const TYPE: EventTypeSpec = {
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
  retentionDays: 90,
  dedupeStrategy: 'natural_key',
};

interface RecordedOutput {
  readonly stdout: string[];
  readonly stderr: string[];
}

function makeDeps(type: EventTypeSpec | null, output: RecordedOutput): EventDescribeDeps {
  return {
    describeEvent: vi.fn(async () => type),
    write: (line) => output.stdout.push(line),
    writeErr: (line) => output.stderr.push(line),
  };
}

describe('runEventDescribe', () => {
  it('returns exit 0 on a known type id', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = makeDeps(TYPE, out);

    const exitCode = await runEventDescribe('app.user.signed-up.v1', deps);

    expect(exitCode).toBe(0);
  });

  it('prints the type id as a header line', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    await runEventDescribe('app.user.signed-up.v1', makeDeps(TYPE, out));

    expect(out.stdout).toContain(TYPE.id);
  });

  it('renders the schema (Required + property names visible)', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    await runEventDescribe('app.user.signed-up.v1', makeDeps(TYPE, out));

    const printed = out.stdout.join('\n');
    expect(printed).toMatch(/^Required:$/m);
    expect(out.stdout.some((l) => l.includes('email'))).toBe(true);
    expect(out.stdout.some((l) => l.includes('plan'))).toBe(true);
  });

  it('renders the dedupe strategy with a "Dedupe:" label', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    await runEventDescribe('app.user.signed-up.v1', makeDeps(TYPE, out));

    expect(out.stdout).toContain('Dedupe: natural_key');
  });

  it('renders the retention period with a "Retention:" label and "days" unit', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    await runEventDescribe('app.user.signed-up.v1', makeDeps(TYPE, out));

    expect(out.stdout).toContain('Retention: 90 days');
  });

  it('renders an "Example:" block', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    await runEventDescribe('app.user.signed-up.v1', makeDeps(TYPE, out));

    expect(out.stdout).toContain('Example:');
  });

  it('omits the "Dedupe:" line when the type has no dedupe strategy', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const minimal: EventTypeSpec = {
      ...TYPE,
      dedupeStrategy: undefined,
    };
    await runEventDescribe('app.user.signed-up.v1', makeDeps(minimal, out));

    expect(out.stdout.some((l) => l.startsWith('Dedupe:'))).toBe(false);
  });

  it('omits the "Retention:" line when the type has no retention setting', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const minimal: EventTypeSpec = {
      ...TYPE,
      retentionDays: undefined,
    };
    await runEventDescribe('app.user.signed-up.v1', makeDeps(minimal, out));

    expect(out.stdout.some((l) => l.startsWith('Retention:'))).toBe(false);
  });

  it('prints the synthesised example using JSON.stringify (so callers can copy it)', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = makeDeps(TYPE, out);

    await runEventDescribe('app.user.signed-up.v1', deps);

    // The example payload is a JSON object; first enum value is "free".
    const printed = out.stdout.join('\n');
    expect(printed).toContain('"email": ""');
    expect(printed).toContain('"plan": "free"');
  });

  it('exits non-zero with a clear stderr message when the type id is unknown', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = makeDeps(null, out);

    const exitCode = await runEventDescribe('app.unknown.v1', deps);

    expect(exitCode).not.toBe(0);
    expect(out.stderr.join('\n')).toContain('app.unknown.v1');
    expect(out.stderr.join('\n')).toMatch(/not found|unknown/i);
  });

  it('rejects an empty type id without calling describeEvent', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = makeDeps(TYPE, out);

    const exitCode = await runEventDescribe('', deps);

    expect(exitCode).not.toBe(0);
    expect(deps.describeEvent).not.toHaveBeenCalled();
  });

  it('handles facade errors by writing them to stderr and exiting non-zero', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const describeEvent = vi.fn(async () => {
      throw new Error('boom');
    });
    const deps: EventDescribeDeps = {
      describeEvent,
      write: (line) => out.stdout.push(line),
      writeErr: (line) => out.stderr.push(line),
    };

    const exitCode = await runEventDescribe('any.id', deps);

    expect(exitCode).not.toBe(0);
    expect(out.stderr.join('\n')).toContain('boom');
  });
});
