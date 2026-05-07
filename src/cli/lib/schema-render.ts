import type { JsonSchema } from '../../lib/example-synth.js';

const INDENT = '  ';

function asJsonSchema(value: unknown): JsonSchema | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as JsonSchema)
    : null;
}

function describeArrayItems(items: JsonSchema | undefined): string {
  if (items === undefined) return 'array';
  if (items.enum !== undefined && items.enum.length > 0) {
    return `array of (one of: ${items.enum.map((v) => JSON.stringify(v)).join(', ')})`;
  }
  if (items.type !== undefined) return `array of ${items.type}`;
  return 'array';
}

function describeType(schema: JsonSchema): string {
  if (schema.enum !== undefined && schema.enum.length > 0) {
    const base = schema.type ?? 'enum';
    return `${base} (one of: ${schema.enum.map((v) => JSON.stringify(v)).join(', ')})`;
  }
  if (schema.type === 'array') return describeArrayItems(schema.items);
  return schema.type ?? 'unknown';
}

function renderField(name: string, schema: JsonSchema): string {
  return `${INDENT}${name}: ${describeType(schema)}`;
}

/** Render a JSON Schema as human-readable lines for `nt event describe`.
 *  Accepts `unknown` so callers don't need to cast across the trust
 *  boundary; non-object inputs degrade to a single descriptor line.
 *  Groups properties under Required: / Optional:; omits a header when its
 *  bucket is empty; falls back to "(no fields)" for an empty object. */
export function renderSchema(rawSchema: unknown): string[] {
  const schema = asJsonSchema(rawSchema);
  if (schema === null) return ['(no fields)'];

  if (schema.type !== 'object') {
    // Top-level scalar / array shapes — describe inline so we don't lose
    // signal on a payload that isn't an object (rare; possible).
    if (schema.type !== undefined || schema.enum !== undefined) {
      return [`(value: ${describeType(schema)})`];
    }
    return ['(no fields)'];
  }
  if (schema.properties === undefined) {
    return ['(no fields)'];
  }

  const props = schema.properties;
  const requiredSet = new Set(schema.required ?? []);

  const requiredFields: string[] = [];
  const optionalFields: string[] = [];
  for (const [name, propSchema] of Object.entries(props)) {
    const line = renderField(name, propSchema);
    if (requiredSet.has(name)) {
      requiredFields.push(line);
    } else {
      optionalFields.push(line);
    }
  }

  if (requiredFields.length === 0 && optionalFields.length === 0) {
    return ['(no fields)'];
  }

  const lines: string[] = [];
  if (requiredFields.length > 0) {
    lines.push('Required:', ...requiredFields);
  }
  if (optionalFields.length > 0) {
    lines.push('Optional:', ...optionalFields);
  }
  return lines;
}
