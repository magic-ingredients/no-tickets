import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { resolveInitAuth } from '../init-auth.js';
import * as credentials from '../../sdk/credentials.js';
import * as authServer from '../../sdk/auth-server.js';

vi.mock('../../sdk/credentials.js');
vi.mock('../../sdk/auth-server.js');

beforeEach(() => {
  vi.clearAllMocks();
});

afterEach(() => {
  vi.restoreAllMocks();
});

function stubAuthServerSuccess(token: string, email: string) {
  const closeFn = vi.fn().mockResolvedValue(undefined);
  vi.mocked(authServer.startAuthServer).mockResolvedValue({
    port: 12345,
    callbackPromise: Promise.resolve({ token, email }),
    close: closeFn,
  });
  return closeFn;
}

describe('resolveInitAuth', () => {
  it('returns existing credentials when loadCredentials succeeds', async () => {
    vi.mocked(credentials.loadCredentials).mockReturnValue({
      token: 'nt_session_existing',
      email: 'user@example.com',
      expiresAt: '2099-01-01T00:00:00Z',
    });

    const result = await resolveInitAuth({
      authUrl: 'https://app.no-tickets.com/api/auth/cli',
      openBrowser: vi.fn(),
    });

    expect(result).toEqual({
      token: 'nt_session_existing',
      email: 'user@example.com',
      isNewAuth: false,
    });
    expect(authServer.startAuthServer).not.toHaveBeenCalled();
  });

  it('runs OAuth flow with port + code params and saves the email returned by the server', async () => {
    vi.mocked(credentials.loadCredentials).mockReturnValue(null);
    stubAuthServerSuccess('nt_session_new_token', 'real@user.com');
    vi.mocked(credentials.saveCredentials).mockReturnValue(undefined);

    const openBrowser = vi.fn().mockResolvedValue(undefined);

    const result = await resolveInitAuth({
      authUrl: 'https://app.no-tickets.com/api/auth/cli',
      openBrowser,
    });

    expect(authServer.startAuthServer).toHaveBeenCalledOnce();
    const calledUrl = new URL(openBrowser.mock.calls[0]![0] as string);
    expect(calledUrl.origin + calledUrl.pathname).toBe('https://app.no-tickets.com/api/auth/cli');
    expect(calledUrl.searchParams.get('port')).toBe('12345');
    expect(calledUrl.searchParams.get('callback_port')).toBeNull();
    const code = calledUrl.searchParams.get('code');
    expect(code).toMatch(/^[0-9a-f]{32}$/);

    expect(result).toEqual({
      token: 'nt_session_new_token',
      email: 'real@user.com',
      isNewAuth: true,
    });
  });

  it('passes the same code as expectedState to startAuthServer', async () => {
    vi.mocked(credentials.loadCredentials).mockReturnValue(null);
    stubAuthServerSuccess('nt_session_x', 'a@b.com');
    vi.mocked(credentials.saveCredentials).mockReturnValue(undefined);

    const openBrowser = vi.fn().mockResolvedValue(undefined);

    await resolveInitAuth({
      authUrl: 'https://app.no-tickets.com/api/auth/cli',
      openBrowser,
    });

    const startCall = vi.mocked(authServer.startAuthServer).mock.calls[0]?.[0];
    const sentCode = new URL(openBrowser.mock.calls[0]![0] as string).searchParams.get('code');
    expect(startCall?.expectedState).toBe(sentCode);
  });

  it('saves credentials with the email returned by the auth server (not a placeholder)', async () => {
    vi.mocked(credentials.loadCredentials).mockReturnValue(null);
    stubAuthServerSuccess('nt_session_saved', 'returned@example.com');

    await resolveInitAuth({
      authUrl: 'https://app.no-tickets.com/api/auth/cli',
      openBrowser: vi.fn().mockResolvedValue(undefined),
    });

    const [token, email, expiresAt] = vi.mocked(credentials.saveCredentials).mock.calls[0]!;
    expect(token).toBe('nt_session_saved');
    expect(email).toBe('returned@example.com');
    const sevenDaysMs = 7 * 24 * 60 * 60 * 1000;
    const diff = new Date(expiresAt).getTime() - Date.now();
    expect(diff).toBeGreaterThan(sevenDaysMs - 5000);
    expect(diff).toBeLessThanOrEqual(sevenDaysMs);
  });

  it('throws when OAuth flow times out', async () => {
    vi.mocked(credentials.loadCredentials).mockReturnValue(null);

    const closeFn = vi.fn().mockResolvedValue(undefined);
    vi.mocked(authServer.startAuthServer).mockResolvedValue({
      port: 12345,
      callbackPromise: Promise.reject(new Error('Authentication timed out — no callback received')),
      close: closeFn,
    });

    await expect(
      resolveInitAuth({
        authUrl: 'https://app.no-tickets.com/api/auth/cli',
        openBrowser: vi.fn().mockResolvedValue(undefined),
      }),
    ).rejects.toThrow('timed out');

    expect(closeFn).toHaveBeenCalled();
  });

  it('closes server even when browser open fails', async () => {
    vi.mocked(credentials.loadCredentials).mockReturnValue(null);

    const closeFn = vi.fn().mockResolvedValue(undefined);
    vi.mocked(authServer.startAuthServer).mockResolvedValue({
      port: 12345,
      callbackPromise: new Promise(() => {}),
      close: closeFn,
    });

    const openBrowser = vi.fn().mockRejectedValue(new Error('Failed to open browser'));

    await expect(
      resolveInitAuth({
        authUrl: 'https://app.no-tickets.com/api/auth/cli',
        openBrowser,
      }),
    ).rejects.toThrow('Failed to open browser');

    expect(closeFn).toHaveBeenCalled();
  });

  it('preserves existing query parameters on the auth URL', async () => {
    vi.mocked(credentials.loadCredentials).mockReturnValue(null);
    stubAuthServerSuccess('nt_session_qp', 'a@b.com');
    vi.mocked(credentials.saveCredentials).mockReturnValue(undefined);

    const openBrowser = vi.fn().mockResolvedValue(undefined);

    await resolveInitAuth({
      authUrl: 'https://app.no-tickets.com/api/auth/cli?provider=kinde',
      openBrowser,
    });

    const calledUrl = new URL(openBrowser.mock.calls[0]![0] as string);
    expect(calledUrl.searchParams.get('provider')).toBe('kinde');
    expect(calledUrl.searchParams.get('port')).toBe('12345');
    expect(calledUrl.searchParams.get('code')).toMatch(/^[0-9a-f]{32}$/);
  });

  it('does not call saveCredentials when auth fails', async () => {
    vi.mocked(credentials.loadCredentials).mockReturnValue(null);

    const closeFn = vi.fn().mockResolvedValue(undefined);
    vi.mocked(authServer.startAuthServer).mockResolvedValue({
      port: 12345,
      callbackPromise: Promise.reject(new Error('Auth server closed')),
      close: closeFn,
    });

    await expect(
      resolveInitAuth({
        authUrl: 'https://app.no-tickets.com/api/auth/cli',
        openBrowser: vi.fn().mockResolvedValue(undefined),
      }),
    ).rejects.toThrow();

    expect(credentials.saveCredentials).not.toHaveBeenCalled();
  });

  it('generates a fresh nonce per call', async () => {
    vi.mocked(credentials.loadCredentials).mockReturnValue(null);
    stubAuthServerSuccess('nt_session_a', 'a@b.com');
    vi.mocked(credentials.saveCredentials).mockReturnValue(undefined);
    const openBrowser = vi.fn().mockResolvedValue(undefined);

    await resolveInitAuth({ authUrl: 'https://app.no-tickets.com/api/auth/cli', openBrowser });

    stubAuthServerSuccess('nt_session_b', 'b@c.com');
    await resolveInitAuth({ authUrl: 'https://app.no-tickets.com/api/auth/cli', openBrowser });

    const code1 = new URL(openBrowser.mock.calls[0]![0] as string).searchParams.get('code');
    const code2 = new URL(openBrowser.mock.calls[1]![0] as string).searchParams.get('code');
    expect(code1).not.toBe(code2);
  });
});
