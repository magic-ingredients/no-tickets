import { z } from 'zod';
import type { Client } from '../transport/client.js';
import { mapResponseError } from '../transport/errors.js';

export const eventTypeSpecSchema = z.object({
  id: z.string().min(1),
  domain: z.string().min(1),
  entity: z.string().min(1),
  action: z.string().min(1),
  version: z.number().int().positive(),
  schema: z.record(z.string(), z.unknown()),
  uiHints: z.record(z.string(), z.unknown()).optional(),
  retentionDays: z.number().int().nonnegative().optional(),
  dedupeStrategy: z.string().optional(),
  deprecatedAt: z.string().nullable().optional(),
});

export type EventTypeSpec = Readonly<z.infer<typeof eventTypeSpecSchema>>;

const listResponseSchema = z.object({
  types: z.array(eventTypeSpecSchema),
});

const idSchema = z.string().min(1);

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
  try {
    return await response.json();
  } catch {
    return null;
  }
}

export async function listEventTypes(
  client: Client,
  options: ListEventTypesOptions = {},
): Promise<ListEventTypesResult> {
  const headers: Record<string, string> = {};
  if (options.ifNoneMatch !== undefined) headers['if-none-match'] = options.ifNoneMatch;

  const response = await client.fetchRaw('GET', buildListPath(options), { headers });

  if (response.status === 304) {
    return { etag: response.headers.get('etag') ?? '', status: 304 };
  }

  if (!response.ok) {
    throw mapResponseError(response.status, await readJson(response));
  }

  const etag = response.headers.get('etag');
  if (etag === null) {
    throw new Error('registry list response missing ETag header');
  }

  const parsed = listResponseSchema.parse(await response.json());
  return { etag, types: parsed.types };
}

export async function getEventType(client: Client, id: string): Promise<EventTypeSpec | null> {
  idSchema.parse(id);
  const path = `${LIST_PATH}/${encodeURIComponent(id)}`;
  const response = await client.fetchRaw('GET', path);

  if (response.status === 404) return null;
  if (!response.ok) {
    throw mapResponseError(response.status, await readJson(response));
  }

  return eventTypeSpecSchema.parse(await response.json());
}
