import type { PublishEvent, PublishResponse } from '../../../transport/events.js';
import type { EventTypeSpec } from '../../../registry/client.js';
import type { JsonSchema } from '../../../lib/example-synth.js';
import type { SubjectRef } from '../../../core/subject.js';
import type { Source } from '../../../core/source.js';
import { resolveDataInput } from '../../lib/data-input.js';
import { parseSourceFlags } from '../../lib/source-flags.js';
import { fuzzyMatch } from '../../lib/fuzzy-match.js';

export interface PublishSingleOptions {
  readonly typeId: string;
  readonly data: string;
  readonly subjectType?: string;
  readonly subjectId?: string;
  readonly sourceName?: string;
  readonly sourceAttributes?: readonly string[];
  readonly parent?: string;
  readonly trace?: string;
  readonly dedupeKey?: string;
}

export interface PublishSingleDeps {
  listEvents(): Promise<readonly EventTypeSpec[]>;
  publish(events: readonly PublishEvent[]): Promise<PublishResponse>;
  readStdin(): Promise<string>;
  write(line: string): void;
  writeErr(line: string): void;
}

const EXIT_OK = 0;
const EXIT_VALIDATION = 1;
const EXIT_UNKNOWN_TYPE = 2;
const EXIT_SERVER = 3;

interface ValidationError {
  readonly path: string;
  readonly message: string;
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

/** Minimal "best-effort" JSON Schema validator.
 *  Handles required-field presence and rough type checks; recurses into
 *  objects and arrays. The server is authoritative — local validation only
 *  catches obvious caller errors. */
function validateAgainstSchema(data: unknown, rawSchema: unknown, path = ''): ValidationError[] {
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
      return errors; // don't double-report with a type error.
    }
    return errors;
  }

  if (!matchesType(data, schema.type)) {
    errors.push({ path: pathLabel(path), message: `expected type ${schema.type ?? 'unknown'}` });
  }

  return errors;
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

function buildSubject(
  subjectType: string | undefined,
  subjectId: string | undefined,
): SubjectRef | undefined {
  if (subjectType !== undefined && subjectId !== undefined) {
    return { type: subjectType, id: subjectId };
  }
  return undefined;
}

export async function runPublishSingle(
  options: PublishSingleOptions,
  deps: PublishSingleDeps,
): Promise<number> {
  if (options.typeId.length === 0) {
    deps.writeErr('publish: <type-id> is required');
    return EXIT_VALIDATION;
  }

  let availableTypes: readonly EventTypeSpec[];
  try {
    availableTypes = await deps.listEvents();
  } catch (err) {
    deps.writeErr(err instanceof Error ? err.message : String(err));
    return EXIT_VALIDATION;
  }

  const typeSpec = availableTypes.find((t) => t.id === options.typeId);
  if (typeSpec === undefined) {
    const suggestions = fuzzyMatch(
      options.typeId,
      availableTypes.map((t) => t.id),
      { topN: 3 },
    );
    deps.writeErr(`Unknown event type: ${options.typeId}`);
    if (suggestions.length > 0) {
      deps.writeErr('Did you mean:');
      for (const s of suggestions) deps.writeErr(`  ${s}`);
    }
    return EXIT_UNKNOWN_TYPE;
  }

  let parsedData: unknown;
  try {
    parsedData = await resolveDataInput(options.data, { readStdin: deps.readStdin });
  } catch (err) {
    deps.writeErr(err instanceof Error ? err.message : String(err));
    return EXIT_VALIDATION;
  }

  const validationErrors = validateAgainstSchema(parsedData, typeSpec.schema);
  if (validationErrors.length > 0) {
    deps.writeErr(`${options.typeId}: ${validationErrors.length} local validation error(s):`);
    for (const e of validationErrors) {
      deps.writeErr(`  ${e.path}: ${e.message}`);
    }
    return EXIT_VALIDATION;
  }

  let source: Partial<Source> | undefined;
  try {
    source = parseSourceFlags({
      ...(options.sourceName !== undefined && { name: options.sourceName }),
      ...(options.sourceAttributes !== undefined && { attributes: options.sourceAttributes }),
    });
  } catch (err) {
    deps.writeErr(err instanceof Error ? err.message : String(err));
    return EXIT_VALIDATION;
  }

  const subject = buildSubject(options.subjectType, options.subjectId);
  const event: PublishEvent = {
    type: options.typeId,
    data: parsedData,
    ...(subject !== undefined && { subject }),
    ...(source !== undefined && { source }),
    ...(options.parent !== undefined && { parentEventId: options.parent }),
    ...(options.trace !== undefined && { traceId: options.trace }),
    ...(options.dedupeKey !== undefined && { dedupeKey: options.dedupeKey }),
  };

  let result: PublishResponse;
  try {
    result = await deps.publish([event]);
  } catch (err) {
    deps.writeErr(err instanceof Error ? err.message : String(err));
    return EXIT_SERVER;
  }

  deps.write(`Published ${result.ingested} event(s); deduped ${result.deduped}.`);
  for (const id of result.ids) deps.write(`  ${id}`);
  return EXIT_OK;
}
