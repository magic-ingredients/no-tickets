import { z } from 'zod';
import type { Client } from './client.js';
import { EventValidationError, type ValidationIssue } from './errors.js';
import { eventSchema, type Event } from '../core/event.js';
import { mergeSource, type Source } from '../core/source.js';

export const publishResponseSchema = z.object({
  ingested: z.number().int().nonnegative(),
  deduped: z.number().int().nonnegative(),
  ids: z.array(z.string().min(1)),
});

export type PublishResponse = Readonly<z.infer<typeof publishResponseSchema>>;

/** Caller-facing input shape — like Event but with `source` optional. publish
 *  auto-fills it from the client's cached Source when omitted, or merges the
 *  partial caller source with the auto-detected one (caller wins on conflicts). */
export type PublishEvent<T = unknown> = Omit<Event<T>, 'source'> & {
  readonly source?: Partial<Source>;
};

const PUBLISH_PATH = '/v1/events';
const EMPTY_RESULT: PublishResponse = { ingested: 0, deduped: 0, ids: [] };

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
  if (events.length === 0) return EMPTY_RESULT;

  const autoSource = client.getSource();
  const enriched = events.map((event) => ({
    ...event,
    source: mergeSource(autoSource, event.source),
  }));

  for (const [i, event] of enriched.entries()) {
    const parsed = eventSchema.safeParse(event);
    if (!parsed.success) {
      throw new EventValidationError(event.type, i, toIssues(parsed.error));
    }
  }

  const response = await client.request<unknown>('POST', PUBLISH_PATH, enriched);
  return publishResponseSchema.parse(response);
}
