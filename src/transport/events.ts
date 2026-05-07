import { z } from 'zod';
import type { Client } from './client.js';
import { EventValidationError, type ValidationIssue } from './errors.js';
import { eventSchema, type Event } from '../core/event.js';
import { mergeSource, type Source } from '../core/source.js';
import { detectSource } from '../agent-detect.js';

export const publishResponseSchema = z.object({
  ingested: z.number().int().nonnegative(),
  deduped: z.number().int().nonnegative(),
  ids: z.array(z.string().min(1)),
});

export type PublishResponse = Readonly<z.infer<typeof publishResponseSchema>>;

/** Caller-facing input shape — like Event but with `source` optional. publish
 *  auto-fills it from detectSource when omitted, or merges the partial caller
 *  source with the auto-detected one when provided (caller wins on conflicts). */
export type PublishEvent<T = unknown> = Omit<Event<T>, 'source'> & {
  readonly source?: Partial<Source>;
};

const PUBLISH_PATH = '/v1/events';

let cachedAutoSource: Source | undefined;

function getAutoSource(): Source {
  if (cachedAutoSource === undefined) {
    cachedAutoSource = detectSource();
  }
  return cachedAutoSource;
}

/** Test-only escape hatch: clear the module-level source cache between cases. */
export function __resetAutoSourceCache(): void {
  cachedAutoSource = undefined;
}

function toIssues(error: z.ZodError): readonly ValidationIssue[] {
  return error.issues.map((issue) => ({
    path: issue.path,
    message: issue.message,
  }));
}

export async function publish(
  client: Client,
  events: readonly PublishEvent[],
): Promise<PublishResponse> {
  const autoSource = getAutoSource();

  const enriched = events.map((event) => ({
    ...event,
    source: mergeSource(autoSource, event.source),
  }));

  for (let i = 0; i < enriched.length; i++) {
    const parsed = eventSchema.safeParse(enriched[i]);
    if (!parsed.success) {
      const typeId = enriched[i]?.type ?? '';
      throw new EventValidationError(typeId, i, toIssues(parsed.error));
    }
  }

  const response = await client.request<unknown>('POST', PUBLISH_PATH, enriched);
  return publishResponseSchema.parse(response);
}
