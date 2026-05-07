import { describe, it, expect, vi } from 'vitest';
import { ZodError } from 'zod';
import { Client } from '../transport/client.js';
import { HttpError } from '../transport/errors.js';
import { listEventTypes, getEventType, type EventTypeSpec } from './client.js';

interface RecordedCall {
  readonly url: string;
  readonly method: string;
  readonly headers: Record<string, string>;
}

interface MockResponseInit {
  readonly status?: number;
  readonly body?: unknown;
  readonly headers?: Record<string, string>;
}

function jsonResponse(init: MockResponseInit = {}): Response {
  const status = init.status ?? 200;
  const bodyText = init.body === undefined ? '' : JSON.stringify(init.body);
  const headers: Record<string, string> = { 'content-type': 'application/json', ...init.headers };
  return new Response(bodyText, { status, headers });
}

function recordingFetch(responses: Response[]): {
  fetch: typeof fetch;
  calls: RecordedCall[];
} {
  const calls: RecordedCall[] = [];
  let i = 0;
  const fetchImpl: typeof fetch = async (input, init) => {
    const url = typeof input === 'string' ? input : input instanceof URL ? input.toString() : input.url;
    const method = (init?.method ?? 'GET').toUpperCase();
    const headers: Record<string, string> = {};
    if (init?.headers) {
      const h = new Headers(init.headers);
      h.forEach((value, key) => {
        headers[key.toLowerCase()] = value;
      });
    }
    calls.push({ url, method, headers });
    const response = responses[i++];
    if (!response) throw new Error(`recordingFetch ran out of responses at call ${i}`);
    return response;
  };
  return { fetch: fetchImpl, calls };
}

const SAMPLE_TYPE: EventTypeSpec = {
  id: 'engineering.deploy.completed.v1',
  domain: 'engineering',
  entity: 'deploy',
  action: 'completed',
  version: 1,
  schema: { type: 'object', properties: {} },
  uiHints: { color: 'green' },
  retentionDays: 90,
  dedupeStrategy: 'natural_key',
  deprecatedAt: null,
};

function client(fetchImpl: typeof fetch): Client {
  return new Client({
    baseUrl: 'https://api.example.com',
    token: 't',
    fetch: fetchImpl,
    source: { name: 'sdk', sdkVersion: '9.9.9-test' },
  });
}

describe('listEventTypes', () => {
  it('GETs /v1/admin/event-types and parses the type array with the etag', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([
      jsonResponse({ body: { types: [SAMPLE_TYPE] }, headers: { etag: 'W/"abc123"' } }),
    ]);

    const result = await listEventTypes(client(fetchImpl));

    expect(calls[0]?.method).toBe('GET');
    expect(calls[0]?.url).toBe('https://api.example.com/v1/admin/event-types');
    expect(calls[0]?.headers['authorization']).toBe('Bearer t');
    expect(result).toEqual({ etag: 'W/"abc123"', types: [SAMPLE_TYPE] });
  });

  it('appends the domain and deprecated query params when supplied', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([
      jsonResponse({ body: { types: [] }, headers: { etag: 'W/"x"' } }),
    ]);

    await listEventTypes(client(fetchImpl), { domain: 'engineering', deprecated: false });

    expect(calls[0]?.url).toBe(
      'https://api.example.com/v1/admin/event-types?domain=engineering&deprecated=false',
    );
  });

  it('omits query params that are not supplied', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([
      jsonResponse({ body: { types: [] }, headers: { etag: 'W/"x"' } }),
    ]);

    await listEventTypes(client(fetchImpl));

    expect(calls[0]?.url).toBe('https://api.example.com/v1/admin/event-types');
  });

  it('sends If-None-Match when ifNoneMatch is provided', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([
      jsonResponse({ body: { types: [] }, headers: { etag: 'W/"new"' } }),
    ]);

    await listEventTypes(client(fetchImpl), { ifNoneMatch: 'W/"prev"' });

    expect(calls[0]?.headers['if-none-match']).toBe('W/"prev"');
  });

  it('returns { etag, status: 304 } when the server responds 304', async () => {
    const r = new Response(null, { status: 304, headers: { etag: 'W/"unchanged"' } });
    const { fetch: fetchImpl } = recordingFetch([r]);

    const result = await listEventTypes(client(fetchImpl), { ifNoneMatch: 'W/"unchanged"' });

    expect(result).toEqual({ etag: 'W/"unchanged"', status: 304 });
  });

  it('reflects a permission-filtered response verbatim (does not re-filter client-side)', async () => {
    // Server already filtered to engineering.* — client passes the array
    // through unchanged even when domain wasn't requested.
    const filtered: EventTypeSpec[] = [SAMPLE_TYPE];
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({ body: { types: filtered }, headers: { etag: 'W/"x"' } }),
    ]);

    const result = await listEventTypes(client(fetchImpl));

    expect('types' in result ? result.types : null).toEqual(filtered);
  });

  it('throws ZodError on a malformed type entry', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({
        body: { types: [{ id: 'x', domain: 'd', entity: 'e' }] }, // missing action/version/schema
        headers: { etag: 'W/"x"' },
      }),
    ]);

    await expect(listEventTypes(client(fetchImpl))).rejects.toBeInstanceOf(ZodError);
  });

  it('throws when the etag header is missing on a 200 response', async () => {
    // ETag is the cache discriminator — without it the cache layer can't
    // do conditional refresh. Surface this loudly rather than caching ''.
    const { fetch: fetchImpl } = recordingFetch([jsonResponse({ body: { types: [] } })]);

    await expect(listEventTypes(client(fetchImpl))).rejects.toThrow(/etag/i);
  });

  it('surfaces non-2xx-non-304 errors via mapResponseError', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({ status: 403, body: { domain: 'engineering' } }),
    ]);

    await expect(listEventTypes(client(fetchImpl))).rejects.toThrow();
  });
});

describe('getEventType', () => {
  it('GETs /v1/admin/event-types/:id and parses the type', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([jsonResponse({ body: SAMPLE_TYPE })]);

    const result = await getEventType(client(fetchImpl), SAMPLE_TYPE.id);

    expect(calls[0]?.method).toBe('GET');
    expect(calls[0]?.url).toBe(
      `https://api.example.com/v1/admin/event-types/${encodeURIComponent(SAMPLE_TYPE.id)}`,
    );
    expect(result).toEqual(SAMPLE_TYPE);
  });

  it('URL-encodes the :id segment', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([jsonResponse({ body: SAMPLE_TYPE })]);

    await getEventType(client(fetchImpl), 'has space/slash');

    expect(calls[0]?.url).toBe(
      'https://api.example.com/v1/admin/event-types/has%20space%2Fslash',
    );
  });

  it('returns null when the server responds 404', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({ status: 404, body: { msg: 'not found' } }),
    ]);

    const result = await getEventType(client(fetchImpl), 'app.unknown.v1');

    expect(result).toBeNull();
  });

  it('rejects an empty id before sending', async () => {
    const fetchImpl = vi.fn();

    await expect(getEventType(client(fetchImpl), '')).rejects.toBeInstanceOf(ZodError);
    expect(fetchImpl).not.toHaveBeenCalled();
  });

  it('throws HttpError on non-404 4xx', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({ status: 401, body: { msg: 'unauthorized' } }),
    ]);

    await expect(getEventType(client(fetchImpl), 'app.x.v1')).rejects.toBeInstanceOf(HttpError);
  });

  it('throws ZodError when the server returns a malformed type', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({ body: { id: 'x', domain: 'd' } }), // missing fields
    ]);

    await expect(getEventType(client(fetchImpl), 'x')).rejects.toBeInstanceOf(ZodError);
  });
});
