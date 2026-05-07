/** Minimal JSON Schema shape we recognise for example synthesis. We don't
 *  validate against a full JSON Schema spec — `nt event describe` and the
 *  matching MCP tool just need a "best-effort" sample payload. */
export interface JsonSchema {
  readonly type?: 'string' | 'number' | 'integer' | 'boolean' | 'null' | 'array' | 'object';
  readonly properties?: Readonly<Record<string, JsonSchema>>;
  readonly required?: readonly string[];
  readonly items?: JsonSchema;
  readonly enum?: readonly unknown[];
  readonly default?: unknown;
}

function placeholderForType(type: JsonSchema['type']): unknown {
  switch (type) {
    case 'string':
      return '';
    case 'number':
    case 'integer':
      return 0;
    case 'boolean':
      return false;
    case 'null':
      return null;
    default:
      return null;
  }
}

function asJsonSchema(value: unknown): JsonSchema | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as JsonSchema)
    : null;
}

/** Synthesise a minimal valid example payload from a JSON Schema fragment.
 *  Accepts `unknown` at the trust boundary so callers don't need to cast;
 *  malformed inputs collapse to `null`.
 *
 *  Resolution order per node: default → enum first value → type placeholder.
 *  Recurses into objects and arrays. Unknown shapes → null. */
export function synthesiseExample(rawSchema: unknown): unknown {
  const schema = asJsonSchema(rawSchema);
  if (schema === null) return null;

  if (Object.prototype.hasOwnProperty.call(schema, 'default')) {
    return schema.default;
  }
  if (schema.enum !== undefined && schema.enum.length > 0) {
    return schema.enum[0];
  }
  if (schema.type === 'object') {
    const result: Record<string, unknown> = {};
    const props = schema.properties ?? {};
    for (const [key, propSchema] of Object.entries(props)) {
      result[key] = synthesiseExample(propSchema);
    }
    return result;
  }
  if (schema.type === 'array') {
    return schema.items !== undefined ? [synthesiseExample(schema.items)] : [];
  }
  if (schema.type === undefined) return null;
  return placeholderForType(schema.type);
}
