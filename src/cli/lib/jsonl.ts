import { readFileSync } from 'node:fs';

export interface JsonlReadDeps {
  readStdin(): Promise<string>;
}

export interface JsonlEntry {
  /** 1-based line number of the source line. */
  readonly line: number;
  readonly value: unknown;
}

/** Read JSONL from a file path, or stdin when path is "-".
 *  Skips blank lines (no phantom record on trailing newline).
 *  Parse failures throw with the failing line number for diagnostics. */
export async function readJsonl(
  path: string,
  deps: JsonlReadDeps,
): Promise<readonly JsonlEntry[]> {
  let raw: string;
  if (path === '-') {
    raw = await deps.readStdin();
  } else {
    try {
      raw = readFileSync(path, 'utf-8');
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      throw new Error(`could not read JSONL file "${path}": ${message}`);
    }
  }

  const result: JsonlEntry[] = [];
  const lines = raw.split('\n');
  for (let i = 0; i < lines.length; i++) {
    // Trim trailing CR for Windows-saved JSONL; treat whitespace-only lines
    // (incl. lone "\r") as blank rather than letting them reach JSON.parse.
    const line = (lines[i] ?? '').replace(/\r$/, '').trim();
    if (line.length === 0) continue;
    const lineNumber = i + 1;
    try {
      result.push({ line: lineNumber, value: JSON.parse(line) });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      throw new Error(`JSONL parse error on line ${lineNumber}: ${message}`);
    }
  }
  return result;
}
