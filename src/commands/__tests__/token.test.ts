import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  createToken,
  listTokens,
  revokeToken,
} from '../token.js';

const TEST_API_URL = 'https://api.no-tickets.com';
const TEST_SESSION_TOKEN = 'nt_session_abc123';

beforeEach(() => {
  vi.clearAllMocks();
  vi.stubGlobal('fetch', vi.fn());
});

afterEach(() => {
  vi.restoreAllMocks();
});

function mockFetch(): ReturnType<typeof vi.fn> {
  return vi.mocked(fetch);
}

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  });
}

describe('createToken', () => {
  it('sends POST to /v1/tokens with project and label', async () => {
    const fetchSpy = mockFetch();
    fetchSpy.mockResolvedValueOnce(
      jsonResponse({ token: 'nt_push_newtoken123', id: 'tok_1' })
    );

    await createToken({
      apiUrl: TEST_API_URL,
      sessionToken: TEST_SESSION_TOKEN,
      projectId: 'proj-xyz',
      label: 'CI deploy',
    });

    expect(fetchSpy).toHaveBeenCalledOnce();
    const [url, options] = fetchSpy.mock.calls[0]!;
    expect(url).toBe(`${TEST_API_URL}/v1/tokens`);
    expect(options.method).toBe('POST');
    expect(options.headers['Authorization']).toBe(`Bearer ${TEST_SESSION_TOKEN}`);
    expect(options.headers['Content-Type']).toBe('application/json');

    const body = JSON.parse(options.body as string) as Record<string, unknown>;
    expect(body['projectId']).toBe('proj-xyz');
    expect(body['label']).toBe('CI deploy');
  });

  it('returns the created token details on success', async () => {
    mockFetch().mockResolvedValueOnce(
      jsonResponse({ token: 'nt_push_newtoken123', id: 'tok_1' })
    );

    const result = await createToken({
      apiUrl: TEST_API_URL,
      sessionToken: TEST_SESSION_TOKEN,
      projectId: 'proj-xyz',
      label: 'CI deploy',
    });

    expect(result).toEqual({
      success: true,
      token: 'nt_push_newtoken123',
      id: 'tok_1',
    });
  });

  it('returns failure on non-ok response', async () => {
    mockFetch().mockResolvedValueOnce(
      jsonResponse({ error: 'Forbidden' }, 403)
    );

    const result = await createToken({
      apiUrl: TEST_API_URL,
      sessionToken: TEST_SESSION_TOKEN,
      projectId: 'proj-xyz',
      label: 'CI deploy',
    });

    expect(result.success).toBe(false);
    expect(result.error).toBe('Forbidden');
  });

  it('returns failure on network error', async () => {
    mockFetch().mockRejectedValueOnce(new Error('Network error'));

    const result = await createToken({
      apiUrl: TEST_API_URL,
      sessionToken: TEST_SESSION_TOKEN,
      projectId: 'proj-xyz',
      label: 'CI deploy',
    });

    expect(result.success).toBe(false);
    expect(result.error).toBe('Network error');
  });

  it('handles non-JSON error response gracefully', async () => {
    mockFetch().mockResolvedValueOnce(
      new Response('<html>502 Bad Gateway</html>', { status: 502 })
    );

    const result = await createToken({
      apiUrl: TEST_API_URL,
      sessionToken: TEST_SESSION_TOKEN,
      projectId: 'proj-xyz',
      label: 'CI deploy',
    });

    expect(result.success).toBe(false);
    expect(result.error).toBe('Request failed with status 502');
  });
});

describe('listTokens', () => {
  it('sends GET to /v1/tokens with auth header', async () => {
    const fetchSpy = mockFetch();
    fetchSpy.mockResolvedValueOnce(jsonResponse({ tokens: [] }));

    await listTokens({
      apiUrl: TEST_API_URL,
      sessionToken: TEST_SESSION_TOKEN,
    });

    expect(fetchSpy).toHaveBeenCalledOnce();
    const [url, options] = fetchSpy.mock.calls[0]!;
    expect(url).toBe(`${TEST_API_URL}/v1/tokens`);
    expect(options.method).toBeUndefined();
    expect(options.headers['Authorization']).toBe(`Bearer ${TEST_SESSION_TOKEN}`);
  });

  it('returns parsed token list on success', async () => {
    mockFetch().mockResolvedValueOnce(
      jsonResponse({
        tokens: [
          { id: 'tok_1', prefix: 'nt_push_abc', label: 'CI', createdAt: '2026-04-16T10:00:00Z' },
          { id: 'tok_2', prefix: 'nt_push_def', label: 'Dev', createdAt: '2026-04-15T08:00:00Z' },
        ],
      })
    );

    const result = await listTokens({
      apiUrl: TEST_API_URL,
      sessionToken: TEST_SESSION_TOKEN,
    });

    expect(result.success).toBe(true);
    expect(result.tokens).toHaveLength(2);
    expect(result.tokens[0]).toEqual({
      id: 'tok_1',
      prefix: 'nt_push_abc',
      label: 'CI',
      createdAt: '2026-04-16T10:00:00Z',
    });
  });

  it('returns failure with error on non-ok response', async () => {
    mockFetch().mockResolvedValueOnce(
      jsonResponse({ error: 'Unauthorized' }, 401)
    );

    const result = await listTokens({
      apiUrl: TEST_API_URL,
      sessionToken: TEST_SESSION_TOKEN,
    });

    expect(result.success).toBe(false);
    expect(result.tokens).toEqual([]);
    expect(result.error).toBe('Unauthorized');
  });

  it('returns failure on network error', async () => {
    mockFetch().mockRejectedValueOnce(new Error('Connection refused'));

    const result = await listTokens({
      apiUrl: TEST_API_URL,
      sessionToken: TEST_SESSION_TOKEN,
    });

    expect(result.success).toBe(false);
    expect(result.tokens).toEqual([]);
    expect(result.error).toBe('Connection refused');
  });

  it('handles malformed token entries gracefully', async () => {
    mockFetch().mockResolvedValueOnce(
      jsonResponse({
        tokens: [
          { id: 'tok_1', prefix: 'nt_push_abc', label: 'CI', createdAt: '2026-04-16T10:00:00Z' },
          null,
          { id: 123 },
        ],
      })
    );

    const result = await listTokens({
      apiUrl: TEST_API_URL,
      sessionToken: TEST_SESSION_TOKEN,
    });

    expect(result.success).toBe(true);
    expect(result.tokens).toHaveLength(2);
    expect(result.tokens[0]!.id).toBe('tok_1');
    expect(result.tokens[1]!.id).toBe('');
  });
});

describe('revokeToken', () => {
  it('sends DELETE to /v1/tokens/:id', async () => {
    const fetchSpy = mockFetch();
    fetchSpy.mockResolvedValueOnce(jsonResponse({ success: true }));

    await revokeToken({
      apiUrl: TEST_API_URL,
      sessionToken: TEST_SESSION_TOKEN,
      tokenId: 'tok_1',
    });

    expect(fetchSpy).toHaveBeenCalledOnce();
    const [url, options] = fetchSpy.mock.calls[0]!;
    expect(url).toBe(`${TEST_API_URL}/v1/tokens/tok_1`);
    expect(options.method).toBe('DELETE');
    expect(options.headers['Authorization']).toBe(`Bearer ${TEST_SESSION_TOKEN}`);
  });

  it('returns success on ok response', async () => {
    mockFetch().mockResolvedValueOnce(jsonResponse({ success: true }));

    const result = await revokeToken({
      apiUrl: TEST_API_URL,
      sessionToken: TEST_SESSION_TOKEN,
      tokenId: 'tok_1',
    });

    expect(result).toEqual({ success: true });
  });

  it('returns failure on non-ok response', async () => {
    mockFetch().mockResolvedValueOnce(
      jsonResponse({ error: 'Token not found' }, 404)
    );

    const result = await revokeToken({
      apiUrl: TEST_API_URL,
      sessionToken: TEST_SESSION_TOKEN,
      tokenId: 'tok_nonexistent',
    });

    expect(result.success).toBe(false);
    expect(result.error).toBe('Token not found');
  });

  it('returns failure on network error', async () => {
    mockFetch().mockRejectedValueOnce(new Error('Connection refused'));

    const result = await revokeToken({
      apiUrl: TEST_API_URL,
      sessionToken: TEST_SESSION_TOKEN,
      tokenId: 'tok_1',
    });

    expect(result.success).toBe(false);
    expect(result.error).toBe('Connection refused');
  });

  it('handles non-JSON error response gracefully', async () => {
    mockFetch().mockResolvedValueOnce(
      new Response('Internal Server Error', { status: 500 })
    );

    const result = await revokeToken({
      apiUrl: TEST_API_URL,
      sessionToken: TEST_SESSION_TOKEN,
      tokenId: 'tok_1',
    });

    expect(result.success).toBe(false);
    expect(result.error).toBe('Request failed with status 500');
  });
});
