import type { EventTypeSpec } from '../../../registry/client.js';
import { renderSchema } from '../../lib/schema-render.js';
import { synthesiseExample, type JsonSchema } from '../../../lib/example-synth.js';

export interface EventDescribeDeps {
  describeEvent(typeId: string): Promise<EventTypeSpec | null>;
  write(line: string): void;
  writeErr(line: string): void;
}

function renderHeader(type: EventTypeSpec, deps: EventDescribeDeps): void {
  deps.write(type.id);
  if (type.dedupeStrategy !== undefined) {
    deps.write(`Dedupe: ${type.dedupeStrategy}`);
  }
  if (type.retentionDays !== undefined) {
    deps.write(`Retention: ${type.retentionDays} days`);
  }
}

function renderExample(schema: JsonSchema, deps: EventDescribeDeps): void {
  deps.write('Example:');
  const example = synthesiseExample(schema);
  for (const line of JSON.stringify(example, null, 2).split('\n')) {
    deps.write(line);
  }
}

export async function runEventDescribe(
  typeId: string,
  deps: EventDescribeDeps,
): Promise<number> {
  if (typeof typeId !== 'string' || typeId.length === 0) {
    deps.writeErr('event type id must be a non-empty string');
    return 1;
  }

  let type: EventTypeSpec | null;
  try {
    type = await deps.describeEvent(typeId);
  } catch (err) {
    deps.writeErr(err instanceof Error ? err.message : String(err));
    return 1;
  }

  if (type === null) {
    deps.writeErr(`Event type "${typeId}" not found.`);
    return 1;
  }

  renderHeader(type, deps);
  for (const line of renderSchema(type.schema as JsonSchema)) {
    deps.write(line);
  }
  deps.write('');
  renderExample(type.schema as JsonSchema, deps);
  return 0;
}
