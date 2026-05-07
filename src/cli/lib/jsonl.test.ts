import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { readJsonl, type JsonlReadDeps } from './jsonl.js';

let tempDir: string;

beforeEach(() => {
  tempDir = mkdtempSync(join(tmpdir(), 'no-tickets-jsonl-'));
});

afterEach(() => {
  rmSync(tempDir, { recursive: true, force: true });
});

const stdinDeps = (text: string): JsonlReadDeps => ({
  readStdin: vi.fn(async () => text),
});

describe('readJsonl', () => {
  it('reads a JSONL file and returns one parsed value per line with 1-based line numbers', async () => {
    const path = join(tempDir, 'events.jsonl');
    writeFileSync(path, '{"a": 1}\n{"a": 2}\n{"a": 3}\n');

    const result = await readJsonl(path, stdinDeps(''));

    expect(result).toEqual([
      { line: 1, value: { a: 1 } },
      { line: 2, value: { a: 2 } },
      { line: 3, value: { a: 3 } },
    ]);
  });

  it('reads stdin when path is "-"', async () => {
    const deps = stdinDeps('{"from": "stdin"}\n{"second": true}\n');

    const result = await readJsonl('-', deps);

    expect(result.map((r) => r.value)).toEqual([{ from: 'stdin' }, { second: true }]);
    expect(deps.readStdin).toHaveBeenCalledTimes(1);
  });

  it('skips blank lines', async () => {
    const path = join(tempDir, 'blanks.jsonl');
    writeFileSync(path, '{"a": 1}\n\n{"a": 2}\n');

    const result = await readJsonl(path, stdinDeps(''));

    expect(result.map((r) => r.line)).toEqual([1, 3]);
    expect(result.map((r) => r.value)).toEqual([{ a: 1 }, { a: 2 }]);
  });

  it('throws with the failing line number on a parse error', async () => {
    const path = join(tempDir, 'broken.jsonl');
    writeFileSync(path, '{"a": 1}\n{not json\n{"a": 3}\n');

    await expect(readJsonl(path, stdinDeps(''))).rejects.toThrow(/line 2/);
  });

  it('throws with a descriptive message when the file does not exist', async () => {
    const path = join(tempDir, 'missing.jsonl');
    await expect(readJsonl(path, stdinDeps(''))).rejects.toThrow(
      new RegExp(`could not read JSONL file.*${path}`),
    );
  });

  it('skips Windows-style CRLF lines and whitespace-only lines without trying to parse them', async () => {
    // Trailing \r on a Windows file would otherwise reach JSON.parse('\r')
    // and throw. Real Windows JSONL: every line ends with \r\n.
    const path = join(tempDir, 'crlf.jsonl');
    writeFileSync(path, '{"a": 1}\r\n   \r\n{"a": 2}\r\n');

    const result = await readJsonl(path, stdinDeps(''));

    expect(result.map((r) => r.value)).toEqual([{ a: 1 }, { a: 2 }]);
    expect(result.map((r) => r.line)).toEqual([1, 3]);
  });

  it('returns an empty array for an empty file', async () => {
    const path = join(tempDir, 'empty.jsonl');
    writeFileSync(path, '');

    expect(await readJsonl(path, stdinDeps(''))).toEqual([]);
  });

  it('handles a trailing newline correctly (does not produce a phantom empty record)', async () => {
    const path = join(tempDir, 'trailing.jsonl');
    writeFileSync(path, '{"a": 1}\n');

    expect(await readJsonl(path, stdinDeps(''))).toEqual([{ line: 1, value: { a: 1 } }]);
  });
});
