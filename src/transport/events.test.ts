import { describe, it, expect, vi, beforeEach } from 'vitest';
import { Client } from './client.js';
import { publish, __resetAutoSourceCache } from './events.js';
import { UnknownEventTypeError, EventValidationError, ServerError } from './errors.js';
import type { Source } from '../core/source.js';
import * as agentDetect from '../agent-detect.js';

vi.mock('../agent-detect.js');

interface RecordedCall {
  readonly url: string;
  readonly method: string;
  readonly body: unknown;
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
    const body = typeof init?.body === 'string' ? JSON.parse(init.body) : undefined;
    calls.push({ url, method, body });
    const response = responses[i++];
    if (!response) throw new Error(`recordingFetch ran out of responses at call ${i}`);
    return response;
  };
  return { fetch: fetchImpl, calls };
}

const AUTO_SOURCE: Source = {
  name: 'sdk',
  sdkVersion: '9.9.9-test',
};

beforeEach(() => {
  __resetAutoSourceCache();
  vi.mocked(agentDetect.detectSource).mockReturnValue(AUTO_SOURCE);
});

describe('publish — happy path', () => {
  it('POSTs to /v1/events with the array as the JSON body (no wrapper)', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([
      jsonResponse({ body: { ingested: 1, deduped: 0, ids: ['evt_1'] } }),
    ]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    const result = await publish(client, [
      { type: 'app.user.signed-up.v1', data: { email: 'a@b.c' } },
    ]);

    expect(calls).toHaveLength(1);
    expect(calls[0]?.url).toBe('https://api.example.com/v1/events');
    expect(calls[0]?.method).toBe('POST');
    expect(Array.isArray(calls[0]?.body)).toBe(true);
    expect(result).toEqual({ ingested: 1, deduped: 0, ids: ['evt_1'] });
  });

  it('sends a batch in a single request', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([
      jsonResponse({ body: { ingested: 3, deduped: 0, ids: ['a', 'b', 'c'] } }),
    ]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    await publish(client, [
      { type: 'app.x.v1', data: { n: 1 } },
      { type: 'app.x.v1', data: { n: 2 } },
      { type: 'app.x.v1', data: { n: 3 } },
    ]);

    expect(calls).toHaveLength(1);
    const body = calls[0]?.body as unknown[];
    expect(body).toHaveLength(3);
  });

  it('returns the dedupe count from the server response', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({ body: { ingested: 2, deduped: 5, ids: ['a', 'b'] } }),
    ]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    const result = await publish(client, [{ type: 'app.x.v1', data: {} }]);

    expect(result.deduped).toBe(5);
  });
});

describe('publish — source auto-fill', () => {
  it('fills source from detectSource when caller omits it', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([
      jsonResponse({ body: { ingested: 1, deduped: 0, ids: ['x'] } }),
    ]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    await publish(client, [{ type: 'app.x.v1', data: {} }]);

    const sent = (calls[0]?.body as { source: Source }[])[0];
    expect(sent?.source).toEqual(AUTO_SOURCE);
  });

  it('caches detectSource across publish calls', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({ body: { ingested: 1, deduped: 0, ids: ['a'] } }),
      jsonResponse({ body: { ingested: 1, deduped: 0, ids: ['b'] } }),
    ]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    await publish(client, [{ type: 'app.x.v1', data: {} }]);
    await publish(client, [{ type: 'app.x.v1', data: {} }]);

    expect(vi.mocked(agentDetect.detectSource)).toHaveBeenCalledTimes(1);
  });

  it('merges caller-supplied partial source over the auto-detected one (caller wins)', async () => {
    vi.mocked(agentDetect.detectSource).mockReturnValue({
      name: 'ci',
      sdkVersion: '9.9.9-test',
      attributes: { provider: 'github-actions', runId: '42' },
    });
    const { fetch: fetchImpl, calls } = recordingFetch([
      jsonResponse({ body: { ingested: 1, deduped: 0, ids: ['x'] } }),
    ]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    await publish(client, [
      {
        type: 'app.x.v1',
        data: {},
        source: {
          name: 'tiny-brain',
          sdkVersion: '9.9.9-test',
          attributes: { provider: 'override' },
        },
      },
    ]);

    const sent = (calls[0]?.body as { source: Source }[])[0];
    expect(sent?.source.name).toBe('tiny-brain');
    expect(sent?.source.attributes).toEqual({ provider: 'override', runId: '42' });
  });
});

describe('publish — local validation', () => {
  it('throws EventValidationError before sending when an envelope is invalid; reports the index', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    const events = [
      { type: 'app.x.v1', data: {} },
      { type: '', data: {} },
    ] as Parameters<typeof publish>[1];

    await expect(publish(client, events)).rejects.toMatchObject({
      name: 'EventValidationError',
      batchIndex: 1,
    });
    expect(calls).toHaveLength(0);
  });

  it('aborts on the FIRST invalid envelope (does not report later ones)', async () => {
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: vi.fn() });

    const events = [
      { type: '', data: {} },
      { type: '', data: {} },
    ] as Parameters<typeof publish>[1];

    await expect(publish(client, events)).rejects.toMatchObject({ batchIndex: 0 });
  });
});

describe('publish — server errors', () => {
  it('surfaces the server batchIndex on 422 unknown_event_type', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({
        status: 422,
        body: { code: 'unknown_event_type', typeId: 'app.unknown.v1', batchIndex: 1 },
      }),
    ]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    await expect(
      publish(client, [
        { type: 'app.x.v1', data: {} },
        { type: 'app.unknown.v1', data: {} },
      ]),
    ).rejects.toMatchObject({
      name: 'UnknownEventTypeError',
      typeId: 'app.unknown.v1',
      batchIndex: 1,
    });
  });

  it('does not retry on 5xx (publish makes a single attempt only)', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([
      jsonResponse({ status: 503, body: '' }),
    ]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    await expect(publish(client, [{ type: 'app.x.v1', data: {} }])).rejects.toBeInstanceOf(ServerError);
    expect(calls).toHaveLength(1);
  });

  it('propagates UnknownEventTypeError as-is (subclass of TransportError)', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({
        status: 422,
        body: { code: 'unknown_event_type', typeId: 'app.x.v1', batchIndex: 0 },
      }),
    ]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    await expect(publish(client, [{ type: 'app.x.v1', data: {} }])).rejects.toBeInstanceOf(
      UnknownEventTypeError,
    );
  });
});

describe('publish — response validation', () => {
  it('throws when the server response is malformed', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({ body: { ingested: 'not-a-number' } }),
    ]);
    const client = new Client({ baseUrl: 'https://api.example.com', token: 't', fetch: fetchImpl });

    await expect(publish(client, [{ type: 'app.x.v1', data: {} }])).rejects.toThrow();
  });
});
