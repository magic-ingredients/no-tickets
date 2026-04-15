import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { NoTicketsClient } from '../client.js';
import type { SyncConfig } from '../core/types.js';

const TEST_CONFIG: SyncConfig = {
  teamId: 'team-abc',
  projectId: 'proj-xyz',
  token: 'nt_test_token',
  apiUrl: 'https://api.no-tickets.com',
};

function mockResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status });
}

describe('NoTicketsClient', () => {
  let fetchSpy: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    fetchSpy = vi.fn();
    vi.stubGlobal('fetch', fetchSpy);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('push', () => {
    it('sends snapshot to API with correct URL and headers', async () => {
      fetchSpy.mockResolvedValueOnce(mockResponse({ success: true, changesApplied: 1, eventsGenerated: 2 }));

      const client = new NoTicketsClient(TEST_CONFIG);
      const result = await client.push({
        version: 1, epics: [], pushedAt: '2026-04-05T12:00:00Z',
      });

      expect(fetchSpy).toHaveBeenCalledOnce();
      const url = fetchSpy.mock.calls[0]?.[0] as string;
      const options = fetchSpy.mock.calls[0]?.[1] as RequestInit;
      expect(url).toBe('https://api.no-tickets.com/api/v1/snapshots');
      expect(options.method).toBe('POST');
      expect((options.headers as Record<string, string>)['Authorization']).toBe('Bearer nt_test_token');
      expect((options.headers as Record<string, string>)['Content-Type']).toBe('application/json');
      expect(result.success).toBe(true);
      expect(result.changesApplied).toBe(1);
      expect(result.eventsGenerated).toBe(2);
    });

    it('includes teamId and projectId in payload', async () => {
      fetchSpy.mockResolvedValueOnce(mockResponse({ success: true, changesApplied: 0, eventsGenerated: 0 }));

      const client = new NoTicketsClient(TEST_CONFIG);
      await client.push({ version: 1, epics: [], pushedAt: '' });

      const options = fetchSpy.mock.calls[0]?.[1] as RequestInit;
      const body = JSON.parse(options.body as string) as Record<string, unknown>;
      expect(body['teamId']).toBe('team-abc');
      expect(body['projectId']).toBe('proj-xyz');
      expect(body['snapshot']).toBeDefined();
    });

    it('returns failure result on HTTP error', async () => {
      fetchSpy.mockResolvedValueOnce(mockResponse('error', 500));

      const client = new NoTicketsClient(TEST_CONFIG);
      const result = await client.push({ version: 1, epics: [], pushedAt: '' });

      expect(result.success).toBe(false);
      expect(result.changesApplied).toBe(0);
      expect(result.eventsGenerated).toBe(0);
    });

    it('returns failure result on network error', async () => {
      fetchSpy.mockRejectedValueOnce(new Error('Network error'));

      const client = new NoTicketsClient(TEST_CONFIG);
      const result = await client.push({ version: 1, epics: [], pushedAt: '' });

      expect(result.success).toBe(false);
      expect(result.changesApplied).toBe(0);
      expect(result.eventsGenerated).toBe(0);
    });

    it('handles malformed API response gracefully', async () => {
      fetchSpy.mockResolvedValueOnce(mockResponse({ unexpected: 'data' }));

      const client = new NoTicketsClient(TEST_CONFIG);
      const result = await client.push({ version: 1, epics: [], pushedAt: '' });

      expect(result.success).toBe(false);
      expect(result.changesApplied).toBe(0);
    });
  });

  describe('connect', () => {
    it('verifies team exists via API with auth header', async () => {
      fetchSpy.mockResolvedValueOnce(mockResponse({ id: 'team-abc', name: 'My Team' }));

      const client = new NoTicketsClient(TEST_CONFIG);
      const result = await client.connect('team-abc');

      expect(result.success).toBe(true);
      expect(result.teamName).toBe('My Team');

      const url = fetchSpy.mock.calls[0]?.[0] as string;
      const options = fetchSpy.mock.calls[0]?.[1] as RequestInit;
      expect(url).toBe('https://api.no-tickets.com/api/v1/teams/team-abc');
      expect((options.headers as Record<string, string>)['Authorization']).toBe('Bearer nt_test_token');
    });

    it('does not send Content-Type on GET request', async () => {
      fetchSpy.mockResolvedValueOnce(mockResponse({ name: 'Team' }));

      const client = new NoTicketsClient(TEST_CONFIG);
      await client.connect('team-abc');

      const options = fetchSpy.mock.calls[0]?.[1] as RequestInit;
      expect((options.headers as Record<string, string>)['Content-Type']).toBeUndefined();
    });

    it('returns failure when team does not exist', async () => {
      fetchSpy.mockResolvedValueOnce(mockResponse('Not Found', 404));

      const client = new NoTicketsClient(TEST_CONFIG);
      const result = await client.connect('nonexistent');

      expect(result.success).toBe(false);
    });
  });

  describe('status', () => {
    it('returns connected status with auth header', async () => {
      fetchSpy.mockResolvedValueOnce(mockResponse({ id: 'team-abc', name: 'My Team' }));

      const client = new NoTicketsClient(TEST_CONFIG);
      const result = await client.status();

      expect(result.connected).toBe(true);
      expect(result.teamName).toBe('My Team');

      const options = fetchSpy.mock.calls[0]?.[1] as RequestInit;
      expect((options.headers as Record<string, string>)['Authorization']).toBe('Bearer nt_test_token');
    });

    it('returns disconnected on API error', async () => {
      fetchSpy.mockResolvedValueOnce(mockResponse('Unauthorized', 401));

      const client = new NoTicketsClient(TEST_CONFIG);
      const result = await client.status();

      expect(result.connected).toBe(false);
    });

    it('returns disconnected on network error', async () => {
      fetchSpy.mockRejectedValueOnce(new Error('Network error'));

      const client = new NoTicketsClient(TEST_CONFIG);
      const result = await client.status();

      expect(result.connected).toBe(false);
    });
  });

  describe('taskUpdate', () => {
    it('sends PUT request with correct URL, headers, and body', async () => {
      fetchSpy.mockResolvedValueOnce(mockResponse({
        task: { id: 'task-1', status: 'in_progress', meta: { phase: 'green' } },
      }));

      const client = new NoTicketsClient(TEST_CONFIG);
      const result = await client.taskUpdate('task-1', {
        status: 'in_progress',
        phase: 'green',
      });

      expect(fetchSpy).toHaveBeenCalledOnce();
      const url = fetchSpy.mock.calls[0]?.[0] as string;
      const options = fetchSpy.mock.calls[0]?.[1] as RequestInit;
      expect(url).toBe('https://api.no-tickets.com/api/v1/tasks/task-1');
      expect(options.method).toBe('PUT');
      expect((options.headers as Record<string, string>)['Authorization']).toBe('Bearer nt_test_token');
      expect((options.headers as Record<string, string>)['Content-Type']).toBe('application/json');

      const body = JSON.parse(options.body as string) as Record<string, unknown>;
      expect(body['status']).toBe('in_progress');
      expect(body['phase']).toBe('green');

      expect(result.success).toBe(true);
      expect(result.task?.status).toBe('in_progress');
    });

    it('sends full metadata in body', async () => {
      fetchSpy.mockResolvedValueOnce(mockResponse({
        task: { id: 'task-1', status: 'completed' },
      }));

      const client = new NoTicketsClient(TEST_CONFIG);
      await client.taskUpdate('task-1', {
        status: 'completed',
        phase: 'complete',
        commitSha: 'abc123',
        coverage: 94,
        testsTotal: 12,
        testsPassing: 12,
        reviewVerdict: 'approved',
        documentation: true,
      });

      const options = fetchSpy.mock.calls[0]?.[1] as RequestInit;
      const body = JSON.parse(options.body as string) as Record<string, unknown>;
      expect(body['commitSha']).toBe('abc123');
      expect(body['coverage']).toBe(94);
      expect(body['testsTotal']).toBe(12);
      expect(body['testsPassing']).toBe(12);
      expect(body['reviewVerdict']).toBe('approved');
      expect(body['documentation']).toBe(true);
    });

    it('returns failure on HTTP error', async () => {
      fetchSpy.mockResolvedValueOnce(mockResponse({ error: 'Not found' }, 404));

      const client = new NoTicketsClient(TEST_CONFIG);
      const result = await client.taskUpdate('task-1', { status: 'in_progress' });

      expect(result.success).toBe(false);
      expect(result.error).toBe('Not found');
    });

    it('returns failure on network error', async () => {
      fetchSpy.mockRejectedValueOnce(new Error('Network error'));

      const client = new NoTicketsClient(TEST_CONFIG);
      const result = await client.taskUpdate('task-1', { status: 'in_progress' });

      expect(result.success).toBe(false);
      expect(result.error).toBe('Network error');
    });

    it('handles malformed success response (no task key)', async () => {
      fetchSpy.mockResolvedValueOnce(mockResponse({ unexpected: 'data' }));

      const client = new NoTicketsClient(TEST_CONFIG);
      const result = await client.taskUpdate('task-1', { status: 'in_progress' });

      expect(result.success).toBe(true);
      expect(result.task).toBeUndefined();
    });

    it('handles non-object success response body', async () => {
      fetchSpy.mockResolvedValueOnce(mockResponse(null));

      const client = new NoTicketsClient(TEST_CONFIG);
      const result = await client.taskUpdate('task-1', { status: 'in_progress' });

      expect(result.success).toBe(true);
      expect(result.task).toBeUndefined();
    });

    it('returns generic error when error response has non-string error field', async () => {
      fetchSpy.mockResolvedValueOnce(mockResponse({ error: 42 }, 422));

      const client = new NoTicketsClient(TEST_CONFIG);
      const result = await client.taskUpdate('task-1', { status: 'in_progress' });

      expect(result.success).toBe(false);
      expect(result.error).toBe('Request failed');
    });

    it('does not call response.json() on HTTP error before parseErrorResponse', async () => {
      fetchSpy.mockResolvedValueOnce(mockResponse({ error: 'Forbidden' }, 403));

      const client = new NoTicketsClient(TEST_CONFIG);
      const result = await client.taskUpdate('task-1', { status: 'in_progress' });

      expect(result.success).toBe(false);
      expect(result.error).toBe('Forbidden');
      expect(fetchSpy).toHaveBeenCalledOnce();
    });
  });
});
