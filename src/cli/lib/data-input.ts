import { readFileSync } from 'node:fs';

export interface DataInputDeps {
  readStdin(): Promise<string>;
}

/** Resolve a `--data <json|@file|->` argument to a parsed JSON value.
 *  - "-": read from stdin via deps.readStdin().
 *  - "@<path>": read the file at <path>.
 *  - Otherwise: parse the input string as inline JSON. */
export async function resolveDataInput(
  input: string,
  deps: DataInputDeps,
): Promise<unknown> {
  if (input.length === 0) {
    throw new Error('--data requires a value (inline JSON, "@<path>", or "-")');
  }
  let raw: string;
  if (input === '-') {
    raw = await deps.readStdin();
  } else if (input.startsWith('@')) {
    const path = input.slice(1);
    try {
      raw = readFileSync(path, 'utf-8');
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      throw new Error(`could not read --data file "${path}": ${message}`);
    }
  } else {
    raw = input;
  }
  try {
    return JSON.parse(raw);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    throw new Error(`--data is not valid JSON: ${message}`);
  }
}
