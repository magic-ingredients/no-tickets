import { describe, it, expect, vi } from 'vitest';
import { ZodError } from 'zod';
import { Client } from './client.js';
import { subjects } from './subjects.js';
import { HttpError } from './errors.js';
import type { Subject, SubjectRef } from '../core/subject.js';

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

const SAMPLE_SUBJECT: Subject = {
  type: 'app.user',
  externalId: 'usr_123',
  displayName: 'Ada Lovelace',
  metadata: { plan: 'pro' },
};

function client(fetchImpl: typeof fetch): Client {
  return new Client({
    baseUrl: 'https://api.example.com',
    token: 't',
    fetch: fetchImpl,
    source: { name: 'sdk', sdkVersion: '9.9.9-test' },
  });
}

describe('subjects.create', () => {
  it('POSTs to /v1/subjects with the Subject as the JSON body', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([jsonResponse({ body: SAMPLE_SUBJECT })]);

    const result = await subjects.create(client(fetchImpl), SAMPLE_SUBJECT);

    expect(calls).toHaveLength(1);
    expect(calls[0]?.method).toBe('POST');
    expect(calls[0]?.url).toBe('https://api.example.com/v1/subjects');
    expect(calls[0]?.body).toEqual(SAMPLE_SUBJECT);
    expect(result).toEqual(SAMPLE_SUBJECT);
  });

  it('throws ZodError when the server response is not a Subject', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({ body: { type: 'app.user', externalId: 'usr_123' } }), // missing displayName
    ]);

    await expect(subjects.create(client(fetchImpl), SAMPLE_SUBJECT)).rejects.toBeInstanceOf(ZodError);
  });

  it('rejects an invalid Subject before sending', async () => {
    const fetchImpl = vi.fn();
    const bad = { type: '', externalId: 'usr_123', displayName: 'x' } as unknown as Subject;

    await expect(subjects.create(client(fetchImpl), bad)).rejects.toBeInstanceOf(ZodError);
    expect(fetchImpl).not.toHaveBeenCalled();
  });
});

describe('subjects.get', () => {
  it('GETs /v1/subjects/:type/:id and parses the Subject', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([jsonResponse({ body: SAMPLE_SUBJECT })]);
    const ref: SubjectRef = { type: 'app.user', id: 'usr_123' };

    const result = await subjects.get(client(fetchImpl), ref);

    expect(calls[0]?.method).toBe('GET');
    expect(calls[0]?.url).toBe('https://api.example.com/v1/subjects/app.user/usr_123');
    expect(result).toEqual(SAMPLE_SUBJECT);
  });

  it('URL-encodes the type and id segments to keep slashes / spaces safe', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([jsonResponse({ body: SAMPLE_SUBJECT })]);
    const ref: SubjectRef = { type: 'app/user', id: 'has space' };

    await subjects.get(client(fetchImpl), ref);

    expect(calls[0]?.url).toBe('https://api.example.com/v1/subjects/app%2Fuser/has%20space');
  });

  it('surfaces a 404 from the server as HttpError (not retried)', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([
      jsonResponse({ status: 404, body: { msg: 'not found' } }),
      jsonResponse({ status: 404, body: { msg: 'not found' } }),
      jsonResponse({ status: 404, body: { msg: 'not found' } }),
    ]);
    const ref: SubjectRef = { type: 'app.user', id: 'missing' };

    const failure = subjects.get(client(fetchImpl), ref);
    await expect(failure).rejects.toBeInstanceOf(HttpError);
    await expect(failure).rejects.toMatchObject({ status: 404 });
    expect(calls).toHaveLength(1);
  });

  it('rejects an invalid SubjectRef before sending', async () => {
    const fetchImpl = vi.fn();
    const bad = { type: '', id: 'usr_123' } as unknown as SubjectRef;

    await expect(subjects.get(client(fetchImpl), bad)).rejects.toBeInstanceOf(ZodError);
    expect(fetchImpl).not.toHaveBeenCalled();
  });
});

describe('subjects.list', () => {
  it('GETs /v1/subjects?type=... and parses an array of Subjects', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([
      jsonResponse({ body: [SAMPLE_SUBJECT, SAMPLE_SUBJECT] }),
    ]);

    const result = await subjects.list(client(fetchImpl), { type: 'app.user' });

    expect(calls[0]?.method).toBe('GET');
    expect(calls[0]?.url).toBe('https://api.example.com/v1/subjects?type=app.user');
    expect(result).toEqual([SAMPLE_SUBJECT, SAMPLE_SUBJECT]);
  });

  it('returns an empty array when the server returns []', async () => {
    const { fetch: fetchImpl } = recordingFetch([jsonResponse({ body: [] })]);

    const result = await subjects.list(client(fetchImpl), { type: 'app.user' });

    expect(result).toEqual([]);
  });

  it('rejects an empty type filter before sending (filter validation)', async () => {
    const fetchImpl = vi.fn();

    await expect(
      subjects.list(client(fetchImpl), { type: '' }),
    ).rejects.toBeInstanceOf(ZodError);
    expect(fetchImpl).not.toHaveBeenCalled();
  });

  it('URL-encodes the type filter so reserved chars are safe', async () => {
    const { fetch: fetchImpl, calls } = recordingFetch([jsonResponse({ body: [] })]);

    await subjects.list(client(fetchImpl), { type: 'app/user&v1' });

    expect(calls[0]?.url).toBe('https://api.example.com/v1/subjects?type=app%2Fuser%26v1');
  });

  it('throws ZodError when the server response is not a Subject array', async () => {
    const { fetch: fetchImpl } = recordingFetch([jsonResponse({ body: { not: 'an array' } })]);

    await expect(
      subjects.list(client(fetchImpl), { type: 'app.user' }),
    ).rejects.toBeInstanceOf(ZodError);
  });
});

describe('subjects — round-trip', () => {
  it('create → get returns the same Subject', async () => {
    const { fetch: fetchImpl } = recordingFetch([
      jsonResponse({ body: SAMPLE_SUBJECT }),
      jsonResponse({ body: SAMPLE_SUBJECT }),
    ]);
    const c = client(fetchImpl);

    const created = await subjects.create(c, SAMPLE_SUBJECT);
    const fetched = await subjects.get(c, { type: created.type, id: created.externalId });

    expect(fetched).toEqual(created);
  });
});
