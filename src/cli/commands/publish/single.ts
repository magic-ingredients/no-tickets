import { byTypeId } from '@magic-ingredients/no-tickets-schemas';
import type { PublishEvent, PublishResponse } from '../../../transport/events.js';
import type { SubjectRef } from '../../../core/subject.js';
import type { Source } from '../../../core/source.js';
import { resolveDataInput } from '../../lib/data-input.js';
import { parseSourceFlags } from '../../lib/source-flags.js';
import { fuzzyMatch } from '../../lib/fuzzy-match.js';
import { isKnownEventType, validateEventLocally } from '../../lib/schema-validate.js';

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
  publish(events: readonly PublishEvent[]): Promise<PublishResponse>;
  readStdin(): Promise<string>;
  write(line: string): void;
  writeErr(line: string): void;
}

const EXIT_OK = 0;
const EXIT_VALIDATION = 1;
const EXIT_UNKNOWN_TYPE = 2;
const EXIT_SERVER = 3;

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

  // Type-existence gate runs BEFORE data parsing so a typo in the type id
  // still surfaces as exit code 2 (unknown_event_type) rather than being
  // masked by an exit code 1 (validation/parse error) when the data is
  // also malformed. Mirrors the original op order; pinned by the
  // "unknown type wins over bad JSON" regression test. The guard narrows
  // typeId from `string` to `EventTypeId`, making the validateEventLocally
  // call below structurally type-safe — no post-hoc runtime check needed.
  if (!isKnownEventType(options.typeId)) {
    const knownIds = Object.keys(byTypeId);
    const suggestions = fuzzyMatch(options.typeId, knownIds, { topN: 3 });
    deps.writeErr(`Unknown event type: ${options.typeId}`);
    if (suggestions.length > 0) {
      deps.writeErr('Did you mean:');
      for (const s of suggestions) deps.writeErr(`  ${s}`);
    }
    return EXIT_UNKNOWN_TYPE;
  }
  const knownTypeId = options.typeId; // narrowed to EventTypeId via the guard

  let parsedData: unknown;
  try {
    parsedData = await resolveDataInput(options.data, { readStdin: deps.readStdin });
  } catch (err) {
    deps.writeErr(err instanceof Error ? err.message : String(err));
    return EXIT_VALIDATION;
  }

  // Schema validation against the npm-bundled Zod schemas. Type existence
  // is already proven via isKnownEventType, so validateEventLocally returns
  // just ValidationIssue[] (no union with { unknownType }).
  const issues = validateEventLocally(knownTypeId, parsedData);
  if (issues.length > 0) {
    deps.writeErr(`${options.typeId}: ${issues.length} local validation error(s):`);
    for (const issue of issues) {
      deps.writeErr(`  ${issue.path}: ${issue.message}`);
    }
    return EXIT_VALIDATION;
  }

  // Surface default — spread order pins --source-name as the override.
  let flagsSource: Partial<Source> | undefined;
  try {
    flagsSource = parseSourceFlags({
      ...(options.sourceName !== undefined && { name: options.sourceName }),
      ...(options.sourceAttributes !== undefined && { attributes: options.sourceAttributes }),
    });
  } catch (err) {
    deps.writeErr(err instanceof Error ? err.message : String(err));
    return EXIT_VALIDATION;
  }
  const source: Partial<Source> = { name: 'cli', ...flagsSource };

  const subject = buildSubject(options.subjectType, options.subjectId);
  const event: PublishEvent = {
    type: options.typeId,
    data: parsedData,
    ...(subject !== undefined && { subject }),
    source,
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
