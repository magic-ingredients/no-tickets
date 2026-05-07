import type { PublishEvent, PublishResponse } from '../../../transport/events.js';
import type { EventTypeSpec } from '../../../registry/client.js';
import type { Source } from '../../../core/source.js';
import { UnknownEventTypeError, EventValidationError } from '../../../transport/errors.js';
import { readJsonl, type JsonlEntry } from '../../lib/jsonl.js';
import { parseSourceFlags } from '../../lib/source-flags.js';
import { validateAgainstSchema } from '../../lib/schema-validate.js';

export interface PublishBatchOptions {
  readonly batchPath: string;
  readonly sourceName?: string;
  readonly sourceAttributes?: readonly string[];
}

export interface PublishBatchDeps {
  listEvents(): Promise<readonly EventTypeSpec[]>;
  publish(events: readonly PublishEvent[]): Promise<PublishResponse>;
  readStdin(): Promise<string>;
  write(line: string): void;
  writeErr(line: string): void;
}

const EXIT_OK = 0;
const EXIT_VALIDATION = 1;
const EXIT_SERVER = 3;

interface BatchEvent {
  readonly line: number;
  readonly event: PublishEvent;
}

function isObjectRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function mergeSourceShallow(
  cliSource: Partial<Source> | undefined,
  jsonlSource: unknown,
): Partial<Source> | undefined {
  const existing = isObjectRecord(jsonlSource) ? (jsonlSource as Partial<Source>) : undefined;
  if (cliSource === undefined && existing === undefined) return undefined;
  if (cliSource === undefined) return existing;
  if (existing === undefined) return cliSource;
  // JSONL line wins on top-level fields; attributes are key-merged so a CLI
  // --source-attribute env=prod survives even when the JSONL line carries
  // its own attributes bag.
  const cliAttrs = isObjectRecord(cliSource.attributes) ? cliSource.attributes : undefined;
  const jsonlAttrs = isObjectRecord(existing.attributes) ? existing.attributes : undefined;
  const attributes =
    cliAttrs !== undefined || jsonlAttrs !== undefined
      ? { ...cliAttrs, ...jsonlAttrs }
      : undefined;
  return {
    ...cliSource,
    ...existing,
    ...(attributes !== undefined && { attributes }),
  };
}

function buildPublishEvent(
  raw: Record<string, unknown>,
  source: Partial<Source> | undefined,
): PublishEvent {
  const { source: rawSource, ...rest } = raw;
  const merged = mergeSourceShallow(source, rawSource);
  return {
    ...(rest as PublishEvent),
    ...(merged !== undefined && { source: merged }),
  };
}

export async function runPublishBatch(
  options: PublishBatchOptions,
  deps: PublishBatchDeps,
): Promise<number> {
  let entries: readonly JsonlEntry[];
  try {
    entries = await readJsonl(options.batchPath, { readStdin: deps.readStdin });
  } catch (err) {
    deps.writeErr(err instanceof Error ? err.message : String(err));
    return EXIT_VALIDATION;
  }

  if (entries.length === 0) {
    deps.writeErr(`batch file "${options.batchPath}" is empty`);
    return EXIT_VALIDATION;
  }

  let availableTypes: readonly EventTypeSpec[];
  try {
    availableTypes = await deps.listEvents();
  } catch (err) {
    deps.writeErr(err instanceof Error ? err.message : String(err));
    return EXIT_VALIDATION;
  }
  const typeIndex = new Map(availableTypes.map((t) => [t.id, t]));

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

  const batchEvents: BatchEvent[] = [];
  for (const entry of entries) {
    if (!isObjectRecord(entry.value)) {
      deps.writeErr(`line ${entry.line}: expected an object event`);
      return EXIT_VALIDATION;
    }
    const typeId = entry.value['type'];
    if (typeof typeId !== 'string' || typeId.length === 0) {
      deps.writeErr(`line ${entry.line}: missing or empty "type" field`);
      return EXIT_VALIDATION;
    }
    const spec = typeIndex.get(typeId);
    if (spec === undefined) {
      deps.writeErr(`line ${entry.line}: unknown event type "${typeId}"`);
      return EXIT_VALIDATION;
    }
    const errors = validateAgainstSchema(entry.value['data'], spec.schema);
    if (errors.length > 0) {
      deps.writeErr(`line ${entry.line}: ${errors.length} validation error(s):`);
      for (const e of errors) deps.writeErr(`  ${e.path}: ${e.message}`);
      return EXIT_VALIDATION;
    }
    batchEvents.push({
      line: entry.line,
      event: buildPublishEvent(entry.value, source),
    });
  }

  let result: PublishResponse;
  try {
    result = await deps.publish(batchEvents.map((b) => b.event));
  } catch (err) {
    if (err instanceof UnknownEventTypeError || err instanceof EventValidationError) {
      const lineNumber = batchEvents[err.batchIndex]?.line;
      const lineLabel = lineNumber !== undefined ? `line ${lineNumber}` : `batch index ${err.batchIndex}`;
      deps.writeErr(`${lineLabel}: ${err.message}`);
    } else {
      deps.writeErr(err instanceof Error ? err.message : String(err));
    }
    return EXIT_SERVER;
  }

  deps.write(`Published ${result.ingested} event(s); deduped ${result.deduped}.`);
  for (const id of result.ids) deps.write(`  ${id}`);
  return EXIT_OK;
}
