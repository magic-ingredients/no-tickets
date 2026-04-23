import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { createApiClient } from '../api-client.js';
import type { BoardState, FeedEvent, Push } from '../../core/types.js';

let fetchSpy: ReturnType<typeof vi.fn>;

beforeEach(() => {
  vi.clearAllMocks();
  fetchSpy = vi.fn();
  vi.stubGlobal('fetch', fetchSpy);
});

afterEach(() => {
  vi.restoreAllMocks();
});

function jsonResponse(body: unknown, status = 200) {
  return Promise.resolve(new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  }));
}

function textResponse(text: string, status: number) {
  return Promise.resolve(new Response(text, { status }));
}

function lastFetchCall(): { url: string; init: RequestInit } {
  expect(fetchSpy).toHaveBeenCalled();
  const calls = fetchSpy.mock.calls;
  const last = calls[calls.length - 1] as [string, RequestInit];
  return { url: last[0], init: last[1] };
}

describe('createApiClient', () => {
  it('creates a client with all methods', () => {
    const client = createApiClient({ token: 'nt_push_abc', apiUrl: 'https://api.no-tickets.com' });
    expect(client).toBeDefined();
    expect(client.getBoard).toBeTypeOf('function');
    expect(client.getFeed).toBeTypeOf('function');
    expect(client.createEpic).toBeTypeOf('function');
    expect(client.createFeature).toBeTypeOf('function');
    expect(client.createFix).toBeTypeOf('function');
    expect(client.updateFeature).toBeTypeOf('function');
    expect(client.moveToPhase).toBeTypeOf('function');
    expect(client.assignFeature).toBeTypeOf('function');
    expect(client.breakDown).toBeTypeOf('function');
  });
});

describe('auth header', () => {
  it('sends Bearer token on GET requests', async () => {
    const client = createApiClient({ token: 'nt_push_secret', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ projectId: 'p1', columns: [] }));

    await client.getBoard('p1');

    const { init } = lastFetchCall();
    expect((init.headers as Record<string, string>)['Authorization']).toBe('Bearer nt_push_secret');
  });

  it('sends Bearer token on POST requests', async () => {
    const client = createApiClient({ token: 'nt_push_post', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ id: 'e1' }));

    await client.createEpic({ projectId: 'p1', title: 'Epic' });

    const { init } = lastFetchCall();
    expect((init.headers as Record<string, string>)['Authorization']).toBe('Bearer nt_push_post');
  });

  it('does not send Content-Type on GET requests', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ projectId: 'p1', columns: [] }));

    await client.getBoard('p1');

    const { init } = lastFetchCall();
    expect((init.headers as Record<string, string>)['Content-Type']).toBeUndefined();
  });

  it('sends Content-Type on POST requests', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ id: 'e1' }));

    await client.createEpic({ projectId: 'p1', title: 'Epic' });

    const { init } = lastFetchCall();
    expect((init.headers as Record<string, string>)['Content-Type']).toBe('application/json');
  });
});

describe('getBoard', () => {
  it('calls GET /v1/board/:projectId and returns BoardState', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    const mockBoard: BoardState = { projectId: 'proj1', columns: [] };
    fetchSpy.mockReturnValue(jsonResponse(mockBoard));

    const result = await client.getBoard('proj1');

    const { url } = lastFetchCall();
    expect(url).toBe('https://api.test.com/v1/board/proj1');
    expect(result).toEqual(mockBoard);
  });
});

describe('getFeed', () => {
  it('calls GET /v1/feed/:projectId and returns FeedEvent array', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    const mockEvents: FeedEvent[] = [{
      id: 'e1', eventType: 'feature_created', actorName: 'bot',
      actorType: 'agent', description: 'Created feature', createdAt: '2026-01-01T00:00:00Z',
    }];
    fetchSpy.mockReturnValue(jsonResponse(mockEvents));

    const result = await client.getFeed('proj1');

    const { url } = lastFetchCall();
    expect(url).toBe('https://api.test.com/v1/feed/proj1');
    expect(result).toEqual(mockEvents);
  });
});

describe('createEpic', () => {
  it('calls POST /v1/epics with body', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ id: 'epic-1' }));

    await client.createEpic({ projectId: 'p1', title: 'My Epic', description: 'Desc' });

    const { url, init } = lastFetchCall();
    expect(url).toBe('https://api.test.com/v1/epics');
    expect(init.method).toBe('POST');
    expect(JSON.parse(init.body as string)).toEqual({ projectId: 'p1', title: 'My Epic', description: 'Desc' });
  });
});

describe('createFeature', () => {
  it('calls POST /v1/features with body', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ id: 'feat-1' }));

    await client.createFeature({ projectId: 'p1', epicId: 'e1', title: 'Feat', description: 'Desc' });

    const { url, init } = lastFetchCall();
    expect(url).toBe('https://api.test.com/v1/features');
    expect(init.method).toBe('POST');
    expect(JSON.parse(init.body as string)).toEqual({
      projectId: 'p1', epicId: 'e1', title: 'Feat', description: 'Desc',
    });
  });
});

describe('createFix', () => {
  it('calls POST /v1/fixes with body', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ id: 'fix-1' }));

    await client.createFix({ projectId: 'p1', epicId: 'e1', title: 'Fix', description: 'Desc' });

    const { url, init } = lastFetchCall();
    expect(url).toBe('https://api.test.com/v1/fixes');
    expect(init.method).toBe('POST');
  });
});

describe('updateFeature', () => {
  it('calls PATCH /v1/features/:featureId with body', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ id: 'feat-1' }));

    await client.updateFeature({ projectId: 'p1', featureId: 'f1', title: 'New Title' });

    const { url, init } = lastFetchCall();
    expect(url).toBe('https://api.test.com/v1/features/f1');
    expect(init.method).toBe('PATCH');
    expect(JSON.parse(init.body as string)).toEqual({ projectId: 'p1', title: 'New Title' });
  });
});

describe('moveToPhase', () => {
  it('calls POST /v1/features/:featureId/move with phase', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ id: 'feat-1' }));

    await client.moveToPhase({ projectId: 'p1', featureId: 'f1', phase: 'testing' });

    const { url, init } = lastFetchCall();
    expect(url).toBe('https://api.test.com/v1/features/f1/move');
    expect(init.method).toBe('POST');
    expect(JSON.parse(init.body as string)).toEqual({ projectId: 'p1', phase: 'testing' });
  });
});

describe('assignFeature', () => {
  it('calls POST /v1/features/:featureId/assign with assignee', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ id: 'feat-1' }));

    await client.assignFeature({ projectId: 'p1', featureId: 'f1', assignee: 'alice', assigneeType: 'human' });

    const { url, init } = lastFetchCall();
    expect(url).toBe('https://api.test.com/v1/features/f1/assign');
    expect(init.method).toBe('POST');
    expect(JSON.parse(init.body as string)).toEqual({
      projectId: 'p1', assignee: 'alice', assigneeType: 'human',
    });
  });
});

describe('breakDown', () => {
  it('calls POST /v1/break-down with body', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ tasks: ['task1'] }));

    await client.breakDown({ projectId: 'p1', featureId: 'f1', context: 'extra info' });

    const { url, init } = lastFetchCall();
    expect(url).toBe('https://api.test.com/v1/break-down');
    expect(init.method).toBe('POST');
    expect(JSON.parse(init.body as string)).toEqual({
      projectId: 'p1', featureId: 'f1', context: 'extra info',
    });
  });
});

describe('push', () => {
  it('calls POST /v1/push with Push payload', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ success: true, changesApplied: 2, eventsGenerated: 1 }));

    const payload: Push = {
      projectId: 'proj-1',
      timestamp: '2026-04-22T10:00:00Z',
      work: { entities: [{ id: 'e-1', type: 'epic', title: 'Platform', status: 'not_started' }] },
    };
    await client.push(payload);

    const { url, init } = lastFetchCall();
    expect(url).toBe('https://api.test.com/v1/push');
    expect(init.method).toBe('POST');
    expect(JSON.parse(init.body as string)).toEqual(payload);
  });

  it('returns PushResult from server', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    const serverResponse = { success: true, changesApplied: 3, eventsGenerated: 2 };
    fetchSpy.mockReturnValue(jsonResponse(serverResponse));

    const result = await client.push({ projectId: 'p1', timestamp: '2026-04-22T10:00:00Z' });

    expect(result).toEqual(serverResponse);
  });

  it('sends Bearer auth header', async () => {
    const client = createApiClient({ token: 'nt_push_xyz', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ success: true, changesApplied: 0, eventsGenerated: 0 }));

    await client.push({ projectId: 'p1', timestamp: '2026-04-22T10:00:00Z' });

    const { init } = lastFetchCall();
    expect((init.headers as Record<string, string>)['Authorization']).toBe('Bearer nt_push_xyz');
  });

  it('throws on server error', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ error: 'Invalid payload' }, 400));

    await expect(client.push({ projectId: 'p1', timestamp: '2026-04-22T10:00:00Z' })).rejects.toThrow('400: Invalid payload');
  });
});

describe('path parameter encoding', () => {
  it('encodes projectId in getBoard URL', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ projectId: 'p', columns: [] }));

    await client.getBoard('proj/with spaces');

    const { url } = lastFetchCall();
    expect(url).toBe('https://api.test.com/v1/board/proj%2Fwith%20spaces');
  });

  it('encodes projectId in getFeed URL', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse([]));

    await client.getFeed('proj/../other');

    const { url } = lastFetchCall();
    expect(url).toBe('https://api.test.com/v1/feed/proj%2F..%2Fother');
  });

  it('encodes featureId in updateFeature URL', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ id: 'f1' }));

    await client.updateFeature({ projectId: 'p1', featureId: 'feat/special', title: 'New' });

    const { url } = lastFetchCall();
    expect(url).toBe('https://api.test.com/v1/features/feat%2Fspecial');
  });

  it('encodes featureId in moveToPhase URL', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ id: 'f1' }));

    await client.moveToPhase({ projectId: 'p1', featureId: 'feat/move', phase: 'done' });

    const { url } = lastFetchCall();
    expect(url).toBe('https://api.test.com/v1/features/feat%2Fmove/move');
  });

  it('encodes featureId in assignFeature URL', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ id: 'f1' }));

    await client.assignFeature({ projectId: 'p1', featureId: 'feat/assign', assignee: 'alice', assigneeType: 'human' });

    const { url } = lastFetchCall();
    expect(url).toBe('https://api.test.com/v1/features/feat%2Fassign/assign');
  });
});

describe('path contract (regression guard)', () => {
  // The /api prefix is a Cloudflare Pages edge convention that only applies
  // when the SPA proxies to the API. Direct API callers must never ship that
  // prefix. 2.0.2 shipped with it and broke against live; this test keeps a
  // flat assertion that every request URL is prefix-free.
  it('never includes /api/ in outgoing request URLs', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockImplementation(() => jsonResponse({ projectId: 'p', columns: [] }));

    await client.getBoard('p1');
    await client.getFeed('p1');
    await client.push({ projectId: 'p', timestamp: '2026-04-22T10:00:00Z' });
    await client.createEpic({ projectId: 'p', title: 'E' });
    await client.createFeature({ projectId: 'p', epicId: 'e1', title: 'F' });
    await client.createFix({ projectId: 'p', epicId: 'e1', title: 'Fix' });
    await client.updateFeature({ projectId: 'p', featureId: 'f1', title: 'New' });
    await client.moveToPhase({ projectId: 'p', featureId: 'f1', phase: 'testing' });
    await client.assignFeature({ projectId: 'p', featureId: 'f1', assignee: 'a', assigneeType: 'human' });
    await client.breakDown({ projectId: 'p', featureId: 'f1' });

    for (const call of fetchSpy.mock.calls) {
      const [url] = call as [string];
      expect(url).not.toContain('/api/');
    }
  });
});

describe('error handling', () => {
  it('throws with status and error field from JSON response', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ error: 'Not found' }, 404));

    await expect(client.getBoard('missing')).rejects.toThrow('404: Not found');
  });

  it('uses fallback message when error field is missing', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ detail: 'something' }, 500));

    await expect(client.getBoard('p1')).rejects.toThrow('500: Request failed');
  });

  it('handles non-JSON error responses', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(textResponse('<html>Server Error</html>', 502));

    await expect(client.getBoard('p1')).rejects.toThrow('502: <html>Server Error</html>');
  });

  it('throws on network-level failures', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockRejectedValue(new TypeError('fetch failed'));

    await expect(client.getBoard('p1')).rejects.toThrow('fetch failed');
  });

  it('uses fallback message when error body is null', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse(null, 500));

    await expect(client.getBoard('p1')).rejects.toThrow('500: Request failed');
  });

  it('uses fallback message when error body is an array', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse(['err'], 500));

    await expect(client.getBoard('p1')).rejects.toThrow('500: Request failed');
  });

  it('returns plain text for successful non-JSON response', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(textResponse('OK', 200));

    const result = await client.getBoard('p1');
    expect(result).toBe('OK');
  });

  it('uses fallback message for empty-body error response', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(textResponse('', 503));

    await expect(client.getBoard('p1')).rejects.toThrow('503: Request failed');
  });

  it('truncates JSON error messages longer than 200 chars', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    const longError = 'x'.repeat(300);
    fetchSpy.mockReturnValue(jsonResponse({ error: longError }, 500));

    await expect(client.getBoard('p1')).rejects.toThrow('x'.repeat(200) + '...');
  });

  it('truncates non-JSON error text longer than 200 chars', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    const longText = 'y'.repeat(300);
    fetchSpy.mockReturnValue(textResponse(longText, 500));

    await expect(client.getBoard('p1')).rejects.toThrow('y'.repeat(200) + '...');
  });

  it('does not truncate error messages at exactly 200 chars', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    const exact200 = 'z'.repeat(200);
    fetchSpy.mockReturnValue(jsonResponse({ error: exact200 }, 500));

    await expect(client.getBoard('p1')).rejects.toThrow(`500: ${exact200}`);
  });
});
