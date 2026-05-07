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
  it('prints the type id, schema fields, dedupe strategy, retention, and example payload', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const deps = makeDeps(TYPE, out);

    const exitCode = await runEventDescribe('app.user.signed-up.v1', deps);

    expect(exitCode).toBe(0);
    const printed = out.stdout.join('\n');
    expect(printed).toContain(TYPE.id);
    expect(printed).toMatch(/required/i);
    expect(printed).toContain('email');
    expect(printed).toContain('plan');
    // Renders dedupe + retention.
    expect(printed).toContain('natural_key');
    expect(printed).toContain('90');
    // Renders an example block with the synthesised payload.
    expect(printed).toMatch(/example/i);
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
