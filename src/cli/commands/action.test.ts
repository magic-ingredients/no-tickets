import { describe, it, expect, vi } from 'vitest';
import { runAction, type ActionDeps, type ActionOptions } from './action.js';
import type { InteractionResponse } from '../../core/interaction.js';
import { PermissionDeniedError, HttpError } from '../../transport/errors.js';

const RESPONSE: InteractionResponse = {
  events: [
    { id: 'evt_1', type: 'app.thread.replied.v1' },
    { id: 'evt_2', type: 'app.thread.notified.v1' },
  ],
};

interface RecordedOutput {
  readonly stdout: string[];
  readonly stderr: string[];
}

interface BuildDepsOptions {
  readonly response?: InteractionResponse;
  readonly error?: unknown;
}

function buildDeps(opts: BuildDepsOptions, out: RecordedOutput): {
  deps: ActionDeps;
  runInteraction: ReturnType<typeof vi.fn>;
} {
  const runInteraction = vi.fn(async () => {
    if (opts.error !== undefined) throw opts.error;
    return opts.response ?? RESPONSE;
  });
  const deps: ActionDeps = {
    runInteraction,
    readStdin: vi.fn(async () => ''),
    write: (l) => out.stdout.push(l),
    writeErr: (l) => out.stderr.push(l),
  };
  return { deps, runInteraction };
}

const baseOptions = (id: string, input: string): ActionOptions => ({
  interactionId: id,
  input,
});

describe('runAction — happy path', () => {
  it('forwards interaction id and parsed input to runInteraction', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, runInteraction } = buildDeps({}, out);

    const exit = await runAction(
      baseOptions('app.thread.reply', '{"text": "hi"}'),
      deps,
    );

    expect(exit).toBe(0);
    expect(runInteraction).toHaveBeenCalledWith('app.thread.reply', {
      input: { text: 'hi' },
    });
  });

  it('attaches subject when both --subject-type and --subject-id are provided', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, runInteraction } = buildDeps({}, out);

    await runAction(
      {
        ...baseOptions('app.thread.reply', '{"text": "hi"}'),
        subjectType: 'app.user',
        subjectId: 'usr_1',
      },
      deps,
    );

    const arg = runInteraction.mock.calls[0]?.[1];
    expect(arg).toMatchObject({
      input: { text: 'hi' },
      subject: { type: 'app.user', id: 'usr_1' },
    });
  });

  it('omits subject when only one of subject-type/subject-id is provided', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, runInteraction } = buildDeps({}, out);

    await runAction(
      {
        ...baseOptions('app.thread.reply', '{"text": "hi"}'),
        subjectType: 'app.user',
      },
      deps,
    );

    const arg = runInteraction.mock.calls[0]?.[1];
    expect(arg).not.toHaveProperty('subject');
  });

  it('reads --input from stdin when input is "-"', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, runInteraction } = buildDeps({}, out);
    vi.mocked(deps.readStdin).mockResolvedValueOnce('{"from": "stdin"}');

    await runAction(baseOptions('app.thread.reply', '-'), deps);

    expect(runInteraction).toHaveBeenCalledWith('app.thread.reply', {
      input: { from: 'stdin' },
    });
  });

  it('prints the response event ids one per line', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps({}, out);

    await runAction(baseOptions('app.thread.reply', '{}'), deps);

    expect(out.stdout).toContain('evt_1');
    expect(out.stdout).toContain('evt_2');
  });
});

describe('runAction — input validation', () => {
  it('exits 1 with a clear message when --input is invalid JSON', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, runInteraction } = buildDeps({}, out);

    const exit = await runAction(baseOptions('app.thread.reply', '{not json'), deps);

    expect(exit).toBe(1);
    expect(runInteraction).not.toHaveBeenCalled();
    expect(out.stderr.join('\n')).toMatch(/json/i);
  });

  it('exits 1 when interaction id is empty', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, runInteraction } = buildDeps({}, out);

    const exit = await runAction(baseOptions('', '{}'), deps);

    expect(exit).toBe(1);
    expect(runInteraction).not.toHaveBeenCalled();
    expect(out.stderr.join('\n')).toMatch(/interaction id/i);
  });
});

describe('runAction — server errors', () => {
  it('exits 1 with a "permission denied" message on PermissionDeniedError', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps(
      { error: new PermissionDeniedError('app.thread') },
      out,
    );

    const exit = await runAction(baseOptions('app.thread.reply', '{}'), deps);

    expect(exit).toBe(1);
    expect(out.stderr.join('\n')).toMatch(/permission denied/i);
    expect(out.stderr.join('\n')).toContain('app.thread');
  });

  it('exits 1 with a clear message on 404 (unknown interaction id)', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps(
      { error: new HttpError(404, { msg: 'not found' }) },
      out,
    );

    const exit = await runAction(baseOptions('app.unknown', '{}'), deps);

    expect(exit).toBe(1);
    expect(out.stderr.join('\n')).toMatch(/not found|unknown/i);
    expect(out.stderr.join('\n')).toContain('app.unknown');
  });

  it('exits 3 on a generic server error', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps({ error: new Error('boom') }, out);

    const exit = await runAction(baseOptions('app.thread.reply', '{}'), deps);

    expect(exit).toBe(3);
    expect(out.stderr.join('\n')).toContain('boom');
  });
});

