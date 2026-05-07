import type { JsonSchema } from '../../lib/example-synth.js';

const INDENT = '  ';

function describeType(schema: JsonSchema): string {
  if (schema.enum !== undefined && schema.enum.length > 0) {
    const base = schema.type ?? 'enum';
    return `${base} (one of: ${schema.enum.map((v) => JSON.stringify(v)).join(', ')})`;
  }
  if (schema.type === 'array') {
    if (schema.items?.type !== undefined) {
      return `array of ${schema.items.type}`;
    }
    return 'array';
  }
  return schema.type ?? 'unknown';
}

function renderField(name: string, schema: JsonSchema): string {
  return `${INDENT}${name}: ${describeType(schema)}`;
}

/** Render a JSON Schema as human-readable lines for `nt event describe`.
 *  Groups properties under Required: / Optional:; omits a header when its
 *  bucket is empty; falls back to "(no fields)" for an empty object. */
export function renderSchema(schema: JsonSchema): string[] {
  if (schema.type !== 'object' || schema.properties === undefined) {
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
