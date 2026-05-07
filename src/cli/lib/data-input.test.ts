import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { resolveDataInput, type DataInputDeps } from './data-input.js';

let tempDir: string;

beforeEach(() => {
  tempDir = mkdtempSync(join(tmpdir(), 'no-tickets-data-input-'));
});

afterEach(() => {
  rmSync(tempDir, { recursive: true, force: true });
});

const stdinDeps = (chunks: string[]): DataInputDeps => ({
  readStdin: vi.fn(async () => chunks.join('')),
});

describe('resolveDataInput', () => {
  it('parses an inline JSON object string', async () => {
    const result = await resolveDataInput('{"hello": "world"}', stdinDeps([]));
    expect(result).toEqual({ hello: 'world' });
  });

  it('parses an inline JSON array string', async () => {
    expect(await resolveDataInput('[1, 2, 3]', stdinDeps([]))).toEqual([1, 2, 3]);
  });

  it('parses an inline JSON scalar', async () => {
    expect(await resolveDataInput('42', stdinDeps([]))).toBe(42);
    expect(await resolveDataInput('true', stdinDeps([]))).toBe(true);
    expect(await resolveDataInput('"a string"', stdinDeps([]))).toBe('a string');
  });

  it('reads stdin when input is "-"', async () => {
    const deps = stdinDeps(['{"from": "stdin"}']);

    const result = await resolveDataInput('-', deps);

    expect(result).toEqual({ from: 'stdin' });
    expect(deps.readStdin).toHaveBeenCalledTimes(1);
  });

  it('reads a file when input starts with "@"', async () => {
    const filePath = join(tempDir, 'payload.json');
    writeFileSync(filePath, '{"from": "file"}');

    const result = await resolveDataInput(`@${filePath}`, stdinDeps([]));

    expect(result).toEqual({ from: 'file' });
  });

  it('throws when the file does not exist', async () => {
    const filePath = join(tempDir, 'missing.json');

    await expect(resolveDataInput(`@${filePath}`, stdinDeps([]))).rejects.toThrow(/file/i);
  });

  it('throws on invalid JSON (inline)', async () => {
    await expect(resolveDataInput('{not json', stdinDeps([]))).rejects.toThrow(/json/i);
  });

  it('throws on invalid JSON (stdin)', async () => {
    await expect(resolveDataInput('-', stdinDeps(['{not json']))).rejects.toThrow(/json/i);
  });

  it('throws on invalid JSON (file)', async () => {
    const filePath = join(tempDir, 'bad.json');
    writeFileSync(filePath, '{not json');

    await expect(resolveDataInput(`@${filePath}`, stdinDeps([]))).rejects.toThrow(/json/i);
  });

  it('rejects empty input string', async () => {
    await expect(resolveDataInput('', stdinDeps([]))).rejects.toThrow();
  });
});
