import type { Source } from '../../core/source.js';

export interface SourceFlags {
  readonly name?: string;
  readonly attributes?: readonly string[];
}

function parseAttribute(raw: string): readonly [string, string] {
  const eqIndex = raw.indexOf('=');
  if (eqIndex < 0) {
    throw new Error(`--source-attribute "${raw}" is malformed (expected key=value)`);
  }
  const key = raw.slice(0, eqIndex);
  if (key.length === 0) {
    throw new Error(`--source-attribute "${raw}" has an empty key`);
  }
  const value = raw.slice(eqIndex + 1);
  return [key, value];
}

/** Parse the CLI's `--source-name` and repeatable `--source-attribute key=value`
 *  flags into a Partial<Source>. Returns undefined when no flags were provided
 *  so callers can pass `source: undefined` straight to publish().
 *
 *  Last value wins on duplicate attribute keys (matches CLI override
 *  conventions where later flags supersede earlier ones). */
export function parseSourceFlags(flags: SourceFlags): Partial<Source> | undefined {
  if (flags.name === undefined && (flags.attributes === undefined || flags.attributes.length === 0)) {
    return undefined;
  }

  const result: { name?: string; attributes?: Record<string, string> } = {};
  if (flags.name !== undefined) result.name = flags.name;

  if (flags.attributes !== undefined && flags.attributes.length > 0) {
    const attributes: Record<string, string> = {};
    for (const raw of flags.attributes) {
      const [key, value] = parseAttribute(raw);
      attributes[key] = value;
    }
    result.attributes = attributes;
  }

  return result;
}
