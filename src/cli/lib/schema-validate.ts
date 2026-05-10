import { byTypeId, type EventTypeId } from '@magic-ingredients/no-tickets-schemas';
import type { JsonSchema } from '../../lib/example-synth.js';

export interface ValidationError {
  readonly path: string;
  readonly message: string;
}

export interface ValidationIssue {
  readonly path: string;
  readonly message: string;
}

/** Type guard — narrows `string` to `EventTypeId` when the id is a
 *  registered event type in the bundled schemas package. Callers gate
 *  publish/validation on this cheaply (no data parsing required) so an
 *  unknown-type error surfaces instead of being masked by a downstream
 *  JSON-parse or schema failure.
 *
 *  Uses Object.hasOwn so prototype names ('toString' / 'hasOwnProperty' /
 *  'valueOf') don't slip past the guard via the prototype chain. */
export function isKnownEventType(typeId: string): typeId is EventTypeId {
  return Object.hasOwn(byTypeId, typeId);
}

/** Validates an event payload against the bundled Zod schema for the
 *  given (already-narrowed) type id. Returns [] on success, ValidationIssue[]
 *  on schema failure.
 *
 *  Pre-condition: caller has gated the type id via `isKnownEventType` —
 *  the `EventTypeId` parameter is the narrowed type returned by that guard.
 *  This makes "unknown event type" structurally impossible at this layer
 *  and removes the dead branch a post-hoc runtime check would create. */
export function validateEventLocally(
  typeId: EventTypeId,
  data: unknown,
): readonly ValidationIssue[] {
  const schema = byTypeId[typeId];
  const result = schema.safeParse(data);
  if (result.success) return [];
  return result.error.issues.map((issue) => ({
    path: issue.path.join('.'),
    message: issue.message,
  }));
}

function asJsonSchema(value: unknown): JsonSchema | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as JsonSchema)
    : null;
}

function pathLabel(path: string, key?: string | number): string {
  if (key === undefined) return path === '' ? '<root>' : path;
  return path === '' ? String(key) : `${path}.${key}`;
}

function matchesType(value: unknown, type: JsonSchema['type']): boolean {
  switch (type) {
    case undefined:
      return true;
    case 'string':
      return typeof value === 'string';
    case 'number':
      return typeof value === 'number';
    case 'integer':
      return typeof value === 'number' && Number.isInteger(value);
    case 'boolean':
      return typeof value === 'boolean';
    case 'null':
      return value === null;
    case 'array':
      return Array.isArray(value);
    case 'object':
      return typeof value === 'object' && value !== null && !Array.isArray(value);
    default:
      return true;
  }
}

/** Best-effort JSON Schema validator: required-field presence + type checks
 *  + enum membership + array-item recursion. Server is authoritative — we
 *  only catch obvious caller errors. Accepts `unknown` at the trust
 *  boundary; non-record schemas degrade to "no errors".
 *
 *  Legacy: still consumed by batch.ts and the MCP discovery-flow smoke
 *  test; deletion lands once those call sites migrate to the bundled-Zod
 *  path. */
export function validateAgainstSchema(
  data: unknown,
  rawSchema: unknown,
  path = '',
): readonly ValidationError[] {
  const schema = asJsonSchema(rawSchema);
  if (schema === null) return [];

  const errors: ValidationError[] = [];

  if (schema.type === 'object') {
    if (typeof data !== 'object' || data === null || Array.isArray(data)) {
      errors.push({ path: pathLabel(path), message: 'expected object' });
      return errors;
    }
    const obj = data as Record<string, unknown>;
    for (const required of schema.required ?? []) {
      if (!(required in obj)) {
        errors.push({
          path: pathLabel(path, required),
          message: `required field "${required}" is missing`,
        });
      }
    }
    for (const [key, propSchema] of Object.entries(schema.properties ?? {})) {
      if (key in obj) {
        errors.push(...validateAgainstSchema(obj[key], propSchema, pathLabel(path, key)));
      }
    }
    return errors;
  }

  if (schema.type === 'array') {
    if (!Array.isArray(data)) {
      errors.push({ path: pathLabel(path), message: 'expected array' });
      return errors;
    }
    if (schema.items !== undefined) {
      data.forEach((item, idx) => {
        errors.push(...validateAgainstSchema(item, schema.items, pathLabel(path, idx)));
      });
    }
    return errors;
  }

  if (schema.enum !== undefined && schema.enum.length > 0) {
    if (!schema.enum.some((v) => v === data)) {
      errors.push({
        path: pathLabel(path),
        message: `value must be one of ${schema.enum.map((v) => JSON.stringify(v)).join(', ')}`,
      });
    }
    return errors;
  }

  if (!matchesType(data, schema.type)) {
    errors.push({ path: pathLabel(path), message: `expected type ${schema.type ?? 'unknown'}` });
  }

  return errors;
}
