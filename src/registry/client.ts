import { z } from 'zod';
import type { Client } from '../transport/client.js';
import { mapResponseError, MissingEtagError } from '../transport/errors.js';

export const eventTypeSpecSchema = z.object({
  id: z.string().min(1),
  domain: z.string().min(1),
  entity: z.string().min(1),
  action: z.string().min(1),
  version: z.number().int().positive(),
  schema: z
    .record(z.string(), z.unknown())
    .refine((s) => Object.keys(s).length > 0, 'schema must not be empty'),
  uiHints: z.record(z.string(), z.unknown()).optional(),
  retentionDays: z.number().int().nonnegative().optional(),
  dedupeStrategy: z.string().min(1).optional(),
  deprecatedAt: z.string().datetime().nullable().optional(),
});

export type EventTypeSpec = Readonly<z.infer<typeof eventTypeSpecSchema>>;

const listResponseSchema = z.object({
  types: z.array(eventTypeSpecSchema),
});

const LIST_PATH = '/v1/admin/event-types';

export interface ListEventTypesOptions {
  readonly domain?: string;
  readonly deprecated?: boolean;
  readonly ifNoneMatch?: string;
}

export type ListEventTypesResult =
  | { readonly etag: string; readonly types: readonly EventTypeSpec[] }
  | { readonly etag: string; readonly status: 304 };

function buildListPath(options: ListEventTypesOptions): string {
  const params = new URLSearchParams();
  if (options.domain !== undefined) params.set('domain', options.domain);
  if (options.deprecated !== undefined) params.set('deprecated', String(options.deprecated));
  const qs = params.toString();
  return qs.length > 0 ? `${LIST_PATH}?${qs}` : LIST_PATH;
}

async function readJson(response: Response): Promise<unknown> {
  const text = await response.text();
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function requireEtag(response: Response, path: string): string {
  const etag = response.headers.get('etag');
  if (etag === null) throw new MissingEtagError(path);
  return etag;
}

// Registry list/describe deliberately do NOT retry on 5xx. The PRD frames
// refresh as async + non-blocking ("If refresh fails, log a debug-level
// note; never error the user-facing command"), so a transient failure
// leaves the cache untouched and the next invocation retries naturally.
// Pinned by the test "does NOT retry on 5xx".

export async function listEventTypes(
  client: Client,
  options: ListEventTypesOptions = {},
): Promise<ListEventTypesResult> {
  const headers: Record<string, string> = {};
  if (options.ifNoneMatch !== undefined) headers['if-none-match'] = options.ifNoneMatch;

  const path = buildListPath(options);
  const response = await client.fetchRaw('GET', path, { headers });

  if (response.status === 304) {
    return { etag: requireEtag(response, path), status: 304 };
  }

  if (!response.ok) {
    throw mapResponseError(response.status, await readJson(response));
  }

  const etag = requireEtag(response, path);
  const parsed = listResponseSchema.parse(await response.json());
  return { etag, types: parsed.types };
}

export async function getEventType(client: Client, id: string): Promise<EventTypeSpec | null> {
  if (typeof id !== 'string' || id.length === 0) {
    throw new TypeError('event type id must be a non-empty string');
  }
  const path = `${LIST_PATH}/${encodeURIComponent(id)}`;
  const response = await client.fetchRaw('GET', path);

  if (response.status === 404) return null;
  if (!response.ok) {
    throw mapResponseError(response.status, await readJson(response));
  }

  return eventTypeSpecSchema.parse(await response.json());
}
