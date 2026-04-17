import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { createApiClient } from '../api-client.js';
import type { BoardState, FeedEvent } from '../../core/types.js';

let fetchSpy: ReturnType<typeof vi.fn>;

beforeEach(() => {
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
  it('calls GET /api/v1/board/:projectId and returns BoardState', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    const mockBoard: BoardState = { projectId: 'proj1', columns: [] };
    fetchSpy.mockReturnValue(jsonResponse(mockBoard));

    const result = await client.getBoard('proj1');

    const { url } = lastFetchCall();
    expect(url).toBe('https://api.test.com/api/v1/board/proj1');
    expect(result).toEqual(mockBoard);
  });
});

describe('getFeed', () => {
  it('calls GET /api/v1/feed/:projectId and returns FeedEvent array', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    const mockEvents: FeedEvent[] = [{
      id: 'e1', eventType: 'feature_created', actorName: 'bot',
      actorType: 'agent', description: 'Created feature', createdAt: '2026-01-01T00:00:00Z',
    }];
    fetchSpy.mockReturnValue(jsonResponse(mockEvents));

    const result = await client.getFeed('proj1');

    const { url } = lastFetchCall();
    expect(url).toBe('https://api.test.com/api/v1/feed/proj1');
    expect(result).toEqual(mockEvents);
  });
});

describe('createEpic', () => {
  it('calls POST /api/v1/epics with body', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ id: 'epic-1' }));

    await client.createEpic({ projectId: 'p1', title: 'My Epic', description: 'Desc' });

    const { url, init } = lastFetchCall();
    expect(url).toBe('https://api.test.com/api/v1/epics');
    expect(init.method).toBe('POST');
    expect(JSON.parse(init.body as string)).toEqual({ projectId: 'p1', title: 'My Epic', description: 'Desc' });
  });
});

describe('createFeature', () => {
  it('calls POST /api/v1/features with body', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ id: 'feat-1' }));

    await client.createFeature({ projectId: 'p1', epicId: 'e1', title: 'Feat', description: 'Desc' });

    const { url, init } = lastFetchCall();
    expect(url).toBe('https://api.test.com/api/v1/features');
    expect(init.method).toBe('POST');
    expect(JSON.parse(init.body as string)).toEqual({
      projectId: 'p1', epicId: 'e1', title: 'Feat', description: 'Desc',
    });
  });
});

describe('createFix', () => {
  it('calls POST /api/v1/fixes with body', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ id: 'fix-1' }));

    await client.createFix({ projectId: 'p1', epicId: 'e1', title: 'Fix', description: 'Desc' });

    const { url, init } = lastFetchCall();
    expect(url).toBe('https://api.test.com/api/v1/fixes');
    expect(init.method).toBe('POST');
  });
});

describe('updateFeature', () => {
  it('calls PATCH /api/v1/features/:featureId with body', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ id: 'feat-1' }));

    await client.updateFeature({ projectId: 'p1', featureId: 'f1', title: 'New Title' });

    const { url, init } = lastFetchCall();
    expect(url).toBe('https://api.test.com/api/v1/features/f1');
    expect(init.method).toBe('PATCH');
    expect(JSON.parse(init.body as string)).toEqual({ projectId: 'p1', title: 'New Title' });
  });
});

describe('moveToPhase', () => {
  it('calls POST /api/v1/features/:featureId/move with phase', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ id: 'feat-1' }));

    await client.moveToPhase({ projectId: 'p1', featureId: 'f1', phase: 'testing' });

    const { url, init } = lastFetchCall();
    expect(url).toBe('https://api.test.com/api/v1/features/f1/move');
    expect(init.method).toBe('POST');
    expect(JSON.parse(init.body as string)).toEqual({ projectId: 'p1', phase: 'testing' });
  });
});

describe('assignFeature', () => {
  it('calls POST /api/v1/features/:featureId/assign with assignee', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ id: 'feat-1' }));

    await client.assignFeature({ projectId: 'p1', featureId: 'f1', assignee: 'alice', assigneeType: 'human' });

    const { url, init } = lastFetchCall();
    expect(url).toBe('https://api.test.com/api/v1/features/f1/assign');
    expect(init.method).toBe('POST');
    expect(JSON.parse(init.body as string)).toEqual({
      projectId: 'p1', assignee: 'alice', assigneeType: 'human',
    });
  });
});

describe('breakDown', () => {
  it('calls POST /api/v1/break-down with body', async () => {
    const client = createApiClient({ token: 'tok', apiUrl: 'https://api.test.com' });
    fetchSpy.mockReturnValue(jsonResponse({ tasks: ['task1'] }));

    await client.breakDown({ projectId: 'p1', featureId: 'f1', context: 'extra info' });

    const { url, init } = lastFetchCall();
    expect(url).toBe('https://api.test.com/api/v1/break-down');
    expect(init.method).toBe('POST');
    expect(JSON.parse(init.body as string)).toEqual({
      projectId: 'p1', featureId: 'f1', context: 'extra info',
    });
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
});
