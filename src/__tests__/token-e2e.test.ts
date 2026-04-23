import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { createToken, listTokens, revokeToken } from '../commands/token.js';

let fetchSpy: ReturnType<typeof vi.fn>;

function jsonResponse(body: unknown, status = 200) {
  return Promise.resolve(new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  }));
}

beforeEach(() => {
  fetchSpy = vi.fn();
  vi.stubGlobal('fetch', fetchSpy);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('token commands e2e', () => {
  it('createToken calls POST /v1/tokens and returns token', async () => {
    fetchSpy.mockReturnValue(jsonResponse({ token: 'nt_push_newtoken', id: 'tok-1' }));

    const result = await createToken({
      apiUrl: 'https://api.test.com',
      sessionToken: 'nt_session_abc',
      projectId: 'proj-1',
      label: 'CI pipeline',
    });

    expect(result.success).toBe(true);
    expect(result.token).toBe('nt_push_newtoken');
    expect(result.id).toBe('tok-1');

    const [url, init] = fetchSpy.mock.calls[0] as [string, RequestInit];
    expect(url).toBe('https://api.test.com/v1/tokens');
    expect(init.method).toBe('POST');
    expect((init.headers as Record<string, string>)['Authorization']).toBe('Bearer nt_session_abc');
  });

  it('listTokens calls GET /v1/tokens and returns entries', async () => {
    fetchSpy.mockReturnValue(jsonResponse({
      tokens: [
        { id: 'tok-1', prefix: 'nt_push_abc', label: 'CI', createdAt: '2026-04-22T00:00:00Z' },
      ],
    }));

    const result = await listTokens({
      apiUrl: 'https://api.test.com',
      sessionToken: 'nt_session_abc',
    });

    expect(result.success).toBe(true);
    expect(result.tokens).toHaveLength(1);
    expect(result.tokens[0]?.prefix).toBe('nt_push_abc');

    const [url] = fetchSpy.mock.calls[0] as [string];
    expect(url).toBe('https://api.test.com/v1/tokens');
  });

  it('revokeToken calls DELETE /v1/tokens/:id', async () => {
    fetchSpy.mockReturnValue(jsonResponse({ success: true }));

    const result = await revokeToken({
      apiUrl: 'https://api.test.com',
      sessionToken: 'nt_session_abc',
      tokenId: 'tok-1',
    });

    expect(result.success).toBe(true);

    const [url, init] = fetchSpy.mock.calls[0] as [string, RequestInit];
    expect(url).toBe('https://api.test.com/v1/tokens/tok-1');
    expect(init.method).toBe('DELETE');
  });

  it('createToken returns error on API failure', async () => {
    fetchSpy.mockReturnValue(jsonResponse({ error: 'Forbidden' }, 403));

    const result = await createToken({
      apiUrl: 'https://api.test.com',
      sessionToken: 'nt_session_abc',
      projectId: 'proj-1',
      label: 'test',
    });

    expect(result.success).toBe(false);
    expect(result.error).toBe('Forbidden');
  });

  it('all token commands send Bearer auth header', async () => {
    fetchSpy.mockReturnValue(jsonResponse({ tokens: [] }));

    await listTokens({ apiUrl: 'https://api.test.com', sessionToken: 'nt_session_secret' });

    const [, init] = fetchSpy.mock.calls[0] as [string, RequestInit];
    expect((init.headers as Record<string, string>)['Authorization']).toBe('Bearer nt_session_secret');
  });
});
