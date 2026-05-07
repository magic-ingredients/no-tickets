import { describe, it, expect, vi } from 'vitest';
import { Client } from './client.js';
import {
  UnknownEventTypeError,
  PermissionDeniedError,
  ServerError,
} from './errors.js';

interface RecordedCall {
  readonly url: string;
  readonly method: string;
  readonly headers: Record<string, string>;
  readonly body: string | undefined;
}

interface MockResponseInit {
  readonly status?: number;
  readonly body?: unknown;
}

function jsonResponse(init: MockResponseInit = {}): Response {
  const status = init.status ?? 200;
  const bodyText = init.body === undefined ? '' : JSON.stringify(init.body);
  return new Response(bodyText, {
    status,
    headers: { 'content-type': 'application/json' },
  });
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
    const rawHeaders = init?.headers;
    if (rawHeaders) {
      const h = new Headers(rawHeaders);
      h.forEach((value, key) => {
        headers[key.toLowerCase()] = value;
      });
    }
    const body = typeof init?.body === 'string' ? init.body : undefined;
    calls.push({ url, method, headers, body });
    const response = responses[i++];
    if (!response) throw new Error(`recordingFetch ran out of responses at call ${i}`);
    return response;
  };
  return { fetch: fetchImpl, calls };
}

describe('Client.request — auth + tracing', () => {
  it('injects Bearer auth header on every request', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([jsonResponse({ body: { ok: true } })]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 'nt_push_abc', fetch: fetchImpl });

    await client.request('GET', '/v1/subjects');

    expect(calls[0]?.headers['authorization']).toBe('Bearer nt_push_abc');
  });

  it('joins baseUrl and path correctly when baseUrl has trailing slash', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([jsonResponse({ body: {} })]);
    const client = new Client({ baseUrl: 'https://api.example.com/', token: 't', fetch: fetchImpl });

    await client.request('GET', '/v1/subjects');

    expect(calls[0]?.url).toBe('https://api.example.com/v1/subjects');
  });

  it('serialises JSON body and sets content-type', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([jsonResponse({ body: {} })]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    await client.request('POST', '/v1/events', [{ type: 'x' }]);

    expect(calls[0]?.headers['content-type']).toBe('application/json');
    expect(calls[0]?.body).toBe(JSON.stringify([{ type: 'x' }]));
  });

  it('parses JSON response body and returns it', async () => {
    const { fetch: fetchImpl } = recordingFetch([jsonResponse({ body: { ingested: 1 } })]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    const result = await client.request<{ ingested: number }>('POST', '/v1/events', []);

    expect(result).toEqual({ ingested: 1 });
  });

  it('returns undefined for 204 No Content', async () => {
    const empty = new Response(null, { status: 204 });
    const { fetch: fetchImpl } = recordingFetch([empty]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    const result = await client.request('DELETE', '/v1/subjects/x/y');

    expect(result).toBeUndefined();
  });

  it('emits a debug log per request (path, status, latency)', async () => {
    const { fetch: fetchImpl } = recordingFetch([jsonResponse({ body: {} })]);
    const debug = vi.fn();
    const client = new Client({
      baseUrl: 'https://api.example.com',
      token: 't',
      fetch: fetchImpl,
      logger: { debug, warn: vi.fn() },
    });

    await client.request('GET', '/v1/subjects');

    expect(debug).toHaveBeenCalledTimes(1);
    const call = debug.mock.calls[0];
    const payload = call?.[0];
    expect(payload).toMatchObject({
      method: 'GET',
      path: '/v1/subjects',
      status: 200,
    });
    expect(typeof payload.latencyMs).toBe('number');
  });
});

describe('Client.request — error mapping', () => {
  it('throws UnknownEventTypeError on 422 unknown_event_type', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({
        status: 422,
        body: { code: 'unknown_event_type', typeId: 'app.user.signed-up.v1', batchIndex: 2 },
      }),
    ]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    await expect(client.request('POST', '/v1/events', [])).rejects.toBeInstanceOf(
      UnknownEventTypeError,
    );
  });

  it('propagates server batchIndex on 422 unknown_event_type', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({
        status: 422,
        body: { code: 'unknown_event_type', typeId: 'app.x.y.v1', batchIndex: 5 },
      }),
    ]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    await expect(client.request('POST', '/v1/events', [])).rejects.toMatchObject({
      typeId: 'app.x.y.v1',
      batchIndex: 5,
    });
  });

  it('throws EventValidationError on 422 event_validation with issues', async () => {
    const issues = [{ path: ['data', 'email'], message: 'required' }];
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({
        status: 422,
        body: { code: 'event_validation', typeId: 'app.user.signed-up.v1', batchIndex: 0, issues },
      }),
    ]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    await expect(client.request('POST', '/v1/events', [])).rejects.toMatchObject({
      name: 'EventValidationError',
      issues,
      batchIndex: 0,
    });
  });

  it('throws PermissionDeniedError on 403', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({ status: 403, body: { domain: 'app.user' } }),
    ]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    await expect(client.request('GET', '/v1/subjects')).rejects.toMatchObject({
      name: 'PermissionDeniedError',
      domain: 'app.user',
    });
  });

  it('throws ServerError on 500 carrying body', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({ status: 500, body: { code: 'INTERNAL' } }),
      jsonResponse({ status: 500, body: { code: 'INTERNAL' } }),
      jsonResponse({ status: 500, body: { code: 'INTERNAL' } }),
    ]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    await expect(client.request('GET', '/v1/subjects')).rejects.toMatchObject({
      name: 'ServerError',
      status: 500,
      body: { code: 'INTERNAL' },
    });
  });
});

describe('Client.request — retry policy', () => {
  it('does NOT retry POST /v1/events on 5xx (single attempt)', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([
      jsonResponse({ status: 503, body: 'unavailable' }),
    ]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    await expect(client.request('POST', '/v1/events', [])).rejects.toBeInstanceOf(ServerError);

    expect(calls).toHaveLength(1);
  });

  it('retries idempotent GET on 5xx up to 3 attempts then throws', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([
      jsonResponse({ status: 502, body: '' }),
      jsonResponse({ status: 502, body: '' }),
      jsonResponse({ status: 502, body: '' }),
    ]);
    const sleep = vi.fn().mockResolvedValue(undefined);
    const client = new Client({
      baseUrl: 'https://api.example.com',
      token: 't',
      fetch: fetchImpl,
      sleep,
    });

    await expect(client.request('GET', '/v1/subjects')).rejects.toBeInstanceOf(ServerError);
    expect(calls).toHaveLength(3);
  });

  it('retries idempotent GET on 5xx and returns once it succeeds', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([
      jsonResponse({ status: 502, body: '' }),
      jsonResponse({ status: 200, body: { ok: true } }),
    ]);
    const sleep = vi.fn().mockResolvedValue(undefined);
    const client = new Client({
      baseUrl: 'https://api.example.com',
      token: 't',
      fetch: fetchImpl,
      sleep,
    });

    const result = await client.request<{ ok: boolean }>('GET', '/v1/subjects');

    expect(result).toEqual({ ok: true });
    expect(calls).toHaveLength(2);
  });

  it('sleeps 100ms then 200ms between the three attempts (exponential backoff)', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({ status: 502, body: '' }),
      jsonResponse({ status: 502, body: '' }),
      jsonResponse({ status: 502, body: '' }),
    ]);
    const sleep = vi.fn().mockResolvedValue(undefined);
    const client = new Client({
      baseUrl: 'https://api.example.com',
      token: 't',
      fetch: fetchImpl,
      sleep,
    });

    await expect(client.request('GET', '/v1/subjects')).rejects.toBeInstanceOf(ServerError);

    expect(sleep).toHaveBeenCalledTimes(2);
    expect(sleep.mock.calls[0]?.[0]).toBe(100);
    expect(sleep.mock.calls[1]?.[0]).toBe(200);
  });

  it('does NOT retry GET on 4xx', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([
      jsonResponse({ status: 403, body: { domain: 'app.user' } }),
    ]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    await expect(client.request('GET', '/v1/subjects')).rejects.toBeInstanceOf(PermissionDeniedError);
    expect(calls).toHaveLength(1);
  });

  it('emits a structured warn log on retry', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({ status: 502, body: '' }),
      jsonResponse({ status: 200, body: { ok: true } }),
    ]);
    const warn = vi.fn();
    const sleep = vi.fn().mockResolvedValue(undefined);
    const client = new Client({
      baseUrl: 'https://api.example.com',
      token: 't',
      fetch: fetchImpl,
      sleep,
      logger: { debug: vi.fn(), warn },
    });

    await client.request('GET', '/v1/subjects');

    expect(warn).toHaveBeenCalledTimes(1);
    expect(warn.mock.calls[0]?.[0]).toMatchObject({
      method: 'GET',
      path: '/v1/subjects',
      status: 502,
      attempt: 1,
    });
  });
});
