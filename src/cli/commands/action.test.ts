import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
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
  readStdin: ReturnType<typeof vi.fn>;
} {
  const runInteraction = vi.fn(async () => {
    if (opts.error !== undefined) throw opts.error;
    return opts.response ?? RESPONSE;
  });
  const readStdin = vi.fn(async () => '');
  const deps: ActionDeps = {
    runInteraction,
    readStdin,
    write: (l) => out.stdout.push(l),
    writeErr: (l) => out.stderr.push(l),
  };
  return { deps, runInteraction, readStdin };
}

let tempDir: string;
beforeEach(() => {
  tempDir = mkdtempSync(join(tmpdir(), 'no-tickets-action-'));
});
afterEach(() => {
  rmSync(tempDir, { recursive: true, force: true });
});

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

  it('omits subject when only --subject-type is provided (subject-id missing)', async () => {
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

  it('omits subject when only --subject-id is provided (subject-type missing)', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, runInteraction } = buildDeps({}, out);

    await runAction(
      {
        ...baseOptions('app.thread.reply', '{"text": "hi"}'),
        subjectId: 'usr_1',
      },
      deps,
    );

    const arg = runInteraction.mock.calls[0]?.[1];
    expect(arg).not.toHaveProperty('subject');
  });

  it('reads --input from stdin when input is "-"', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, runInteraction, readStdin } = buildDeps({}, out);
    readStdin.mockResolvedValueOnce('{"from": "stdin"}');

    await runAction(baseOptions('app.thread.reply', '-'), deps);

    expect(runInteraction).toHaveBeenCalledWith('app.thread.reply', {
      input: { from: 'stdin' },
    });
  });

  it('reads --input from a file when input is "@<path>"', async () => {
    const path = join(tempDir, 'input.json');
    writeFileSync(path, '{"from": "file"}');
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, runInteraction } = buildDeps({}, out);

    await runAction(baseOptions('app.thread.reply', `@${path}`), deps);

    expect(runInteraction).toHaveBeenCalledWith('app.thread.reply', {
      input: { from: 'file' },
    });
  });

  it('prints the response event ids EXACTLY once each, in order, on stdout', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps({}, out);

    await runAction(baseOptions('app.thread.reply', '{}'), deps);

    expect(out.stdout).toEqual(['evt_1', 'evt_2']);
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

  it('exits 1 when interaction id is whitespace-only', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps, runInteraction } = buildDeps({}, out);

    const exit = await runAction(baseOptions('   ', '{}'), deps);

    expect(exit).toBe(1);
    expect(runInteraction).not.toHaveBeenCalled();
  });
});

describe('runAction — server errors', () => {
  it('exits 1 with the bespoke `permission denied for domain "<x>"` message on PermissionDeniedError', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps(
      { error: new PermissionDeniedError('app.thread') },
      out,
    );

    const exit = await runAction(baseOptions('app.thread.reply', '{}'), deps);

    expect(exit).toBe(1);
    expect(out.stderr).toContain('permission denied for domain "app.thread"');
  });

  it('exits 1 with a "not found" message including the interaction id on HttpError 404', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps(
      { error: new HttpError(404, { msg: 'not found' }) },
      out,
    );

    const exit = await runAction(baseOptions('app.unknown', '{}'), deps);

    expect(exit).toBe(1);
    expect(out.stderr.join('\n')).toMatch(/not found/);
    expect(out.stderr.join('\n')).toContain('app.unknown');
  });

  it('exits 3 on a non-404 HttpError (e.g. 500) — does NOT downgrade to validation', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps(
      { error: new HttpError(500, { msg: 'internal' }) },
      out,
    );

    const exit = await runAction(baseOptions('app.thread.reply', '{}'), deps);

    expect(exit).toBe(3);
  });

  it('exits 3 on a generic server error', async () => {
    const out: RecordedOutput = { stdout: [], stderr: [] };
    const { deps } = buildDeps({ error: new Error('boom') }, out);

    const exit = await runAction(baseOptions('app.thread.reply', '{}'), deps);

    expect(exit).toBe(3);
    expect(out.stderr.join('\n')).toContain('boom');
  });
});

