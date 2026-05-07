import { describe, it, expect, vi } from 'vitest';
import { ZodError } from 'zod';
import { Client } from './client.js';
import { runInteraction } from './interactions.js';
import { EventValidationError, PermissionDeniedError } from './errors.js';
import type { SubjectRef } from '../core/subject.js';

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
    const rawBody = init?.body;
    let body: unknown = undefined;
    if (rawBody !== undefined && rawBody !== null) {
      if (typeof rawBody !== 'string') {
        throw new Error(
          `recordingFetch only handles string request bodies, got ${typeof rawBody}`,
        );
      }
      body = JSON.parse(rawBody);
    }
    calls.push({ url, method, body });
    const response = responses[i++];
    if (!response) throw new Error(`recordingFetch ran out of responses at call ${i}`);
    return response;
  };
  return { fetch: fetchImpl, calls };
}

const RESPONSE_OK = {
  events: [
    { id: 'evt_1', type: 'app.thread.replied.v1' },
    { id: 'evt_2', type: 'app.thread.notified.v1' },
  ],
};

function client(fetchImpl: typeof fetch): Client {
  return new Client({
    baseUrl: 'https://api.example.com',
    token: 't',
    fetch: fetchImpl,
    source: { name: 'sdk', sdkVersion: '9.9.9-test' },
  });
}

describe('runInteraction — happy path', () => {
  it('POSTs to /v1/interactions/:id with { input } as the JSON body', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([jsonResponse({ body: RESPONSE_OK })]);

    const result = await runInteraction(client(fetchImpl), 'app.thread.reply', {
      input: { threadId: 't_1', text: 'hi' },
    });

    expect(calls).toHaveLength(1);
    expect(calls[0]?.method).toBe('POST');
    expect(calls[0]?.url).toBe('https://api.example.com/v1/interactions/app.thread.reply');
    expect(calls[0]?.body).toEqual({ input: { threadId: 't_1', text: 'hi' } });
    expect(result).toEqual(RESPONSE_OK);
  });

  it('includes subject in the body when provided', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([jsonResponse({ body: RESPONSE_OK })]);
    const subject: SubjectRef = { type: 'app.user', id: 'usr_1' };

    await runInteraction(client(fetchImpl), 'app.thread.reply', {
      input: { text: 'hi' },
      subject,
    });

    expect(calls[0]?.body).toEqual({ input: { text: 'hi' }, subject });
  });

  it('omits subject from the body when not provided', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([jsonResponse({ body: RESPONSE_OK })]);

    await runInteraction(client(fetchImpl), 'app.thread.reply', { input: { text: 'hi' } });

    expect(calls[0]?.body).toEqual({ input: { text: 'hi' } });
    expect(Object.keys(calls[0]?.body as object)).not.toContain('subject');
  });

  it('URL-encodes the :id segment so reserved chars survive', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([jsonResponse({ body: RESPONSE_OK })]);

    await runInteraction(client(fetchImpl), 'app/thread reply', { input: {} });

    expect(calls[0]?.url).toBe('https://api.example.com/v1/interactions/app%2Fthread%20reply');
  });
});

describe('runInteraction — local validation', () => {
  it('rejects an empty id before sending', async () => {
    const fetchImpl = vi.fn();

    await expect(
      runInteraction(client(fetchImpl), '', { input: {} }),
    ).rejects.toBeInstanceOf(ZodError);
    expect(fetchImpl).not.toHaveBeenCalled();
  });

  it('rejects an invalid subject before sending', async () => {
    const fetchImpl = vi.fn();
    const badSubject = { type: '', id: 'usr_1' } as unknown as SubjectRef;

    await expect(
      runInteraction(client(fetchImpl), 'app.thread.reply', { input: {}, subject: badSubject }),
    ).rejects.toBeInstanceOf(ZodError);
    expect(fetchImpl).not.toHaveBeenCalled();
  });
});

describe('runInteraction — server errors', () => {
  it('maps 403 with a domain to PermissionDeniedError', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({ status: 403, body: { domain: 'app.thread' } }),
    ]);

    const failure = runInteraction(client(fetchImpl), 'app.thread.reply', { input: {} });

    await expect(failure).rejects.toBeInstanceOf(PermissionDeniedError);
    await expect(failure).rejects.toMatchObject({ domain: 'app.thread' });
  });

  it('maps 422 event_validation to EventValidationError carrying the issues', async () => {
    const issues = [{ path: ['input', 'text'], message: 'required' }];
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({
        status: 422,
        body: {
          code: 'event_validation',
          typeId: 'app.thread.reply',
          batchIndex: 0,
          issues,
        },
      }),
    ]);

    const failure = runInteraction(client(fetchImpl), 'app.thread.reply', { input: {} });

    await expect(failure).rejects.toBeInstanceOf(EventValidationError);
    await expect(failure).rejects.toMatchObject({
      typeId: 'app.thread.reply',
      issues,
    });
  });
});

describe('runInteraction — response validation', () => {
  it('throws a ZodError when the server response is not an InteractionResponse', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({ body: { events: 'not-an-array' } }),
    ]);

    await expect(
      runInteraction(client(fetchImpl), 'app.thread.reply', { input: {} }),
    ).rejects.toBeInstanceOf(ZodError);
  });

  it('throws a ZodError when an event ref in the response is missing a field', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({ body: { events: [{ id: 'evt_1' }] } }), // missing type
    ]);

    await expect(
      runInteraction(client(fetchImpl), 'app.thread.reply', { input: {} }),
    ).rejects.toBeInstanceOf(ZodError);
  });
});
