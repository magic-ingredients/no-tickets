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

describe('resolveInitAuth', () => {
  it('returns existing credentials when valid and not expired', async () => {
    vi.mocked(credentials.loadCredentials).mockReturnValue({
      token: 'nt_session_existing',
      email: 'user@example.com',
      expiresAt: '2099-01-01T00:00:00Z',
    });

    const result = await resolveInitAuth({
      authUrl: 'https://auth.no-tickets.com/cli',
      openBrowser: vi.fn(),
    });

    expect(result).toEqual({
      token: 'nt_session_existing',
      email: 'user@example.com',
      isNewAuth: false,
    });
    expect(authServer.startAuthServer).not.toHaveBeenCalled();
  });

  it('runs OAuth flow when no credentials exist', async () => {
    vi.mocked(credentials.loadCredentials).mockReturnValue(null);

    const closeFn = vi.fn().mockResolvedValue(undefined);
    vi.mocked(authServer.startAuthServer).mockResolvedValue({
      port: 12345,
      tokenPromise: Promise.resolve('nt_session_new_token'),
      close: closeFn,
    });
    vi.mocked(credentials.saveCredentials).mockReturnValue(undefined);

    const openBrowser = vi.fn().mockResolvedValue(undefined);

    const result = await resolveInitAuth({
      authUrl: 'https://auth.no-tickets.com/cli',
      openBrowser,
    });

    expect(authServer.startAuthServer).toHaveBeenCalledOnce();
    expect(openBrowser).toHaveBeenCalledWith(
      expect.stringContaining('https://auth.no-tickets.com/cli')
    );
    expect(openBrowser).toHaveBeenCalledWith(
      expect.stringContaining('callback_port=12345')
    );
    expect(credentials.saveCredentials).toHaveBeenCalledOnce();
    expect(result.token).toBe('nt_session_new_token');
    expect(result.isNewAuth).toBe(true);
  });

  it('saves credentials after successful OAuth flow', async () => {
    vi.mocked(credentials.loadCredentials).mockReturnValue(null);

    const closeFn = vi.fn().mockResolvedValue(undefined);
    vi.mocked(authServer.startAuthServer).mockResolvedValue({
      port: 12345,
      tokenPromise: Promise.resolve('nt_session_saved'),
      close: closeFn,
    });

    await resolveInitAuth({
      authUrl: 'https://auth.no-tickets.com/cli',
      openBrowser: vi.fn().mockResolvedValue(undefined),
    });

    const [token, email, expiresAt] = vi.mocked(credentials.saveCredentials).mock.calls[0]!;
    expect(token).toBe('nt_session_saved');
    expect(typeof email).toBe('string');
    expect(typeof expiresAt).toBe('string');
  });

  it('throws when OAuth flow times out', async () => {
    vi.mocked(credentials.loadCredentials).mockReturnValue(null);

    const closeFn = vi.fn().mockResolvedValue(undefined);
    vi.mocked(authServer.startAuthServer).mockResolvedValue({
      port: 12345,
      tokenPromise: Promise.reject(new Error('Authentication timed out — no callback received')),
      close: closeFn,
    });

    await expect(
      resolveInitAuth({
        authUrl: 'https://auth.no-tickets.com/cli',
        openBrowser: vi.fn().mockResolvedValue(undefined),
      })
    ).rejects.toThrow('timed out');

    expect(closeFn).toHaveBeenCalled();
  });

  it('closes server even when browser open fails', async () => {
    vi.mocked(credentials.loadCredentials).mockReturnValue(null);

    const closeFn = vi.fn().mockResolvedValue(undefined);
    vi.mocked(authServer.startAuthServer).mockResolvedValue({
      port: 12345,
      tokenPromise: new Promise(() => {}), // never resolves
      close: closeFn,
    });

    const openBrowser = vi.fn().mockRejectedValue(new Error('Failed to open browser'));

    await expect(
      resolveInitAuth({
        authUrl: 'https://auth.no-tickets.com/cli',
        openBrowser,
      })
    ).rejects.toThrow('Failed to open browser');

    expect(closeFn).toHaveBeenCalled();
  });

  it('does not call saveCredentials when auth fails', async () => {
    vi.mocked(credentials.loadCredentials).mockReturnValue(null);

    const closeFn = vi.fn().mockResolvedValue(undefined);
    vi.mocked(authServer.startAuthServer).mockResolvedValue({
      port: 12345,
      tokenPromise: Promise.reject(new Error('Auth server closed')),
      close: closeFn,
    });

    await expect(
      resolveInitAuth({
        authUrl: 'https://auth.no-tickets.com/cli',
        openBrowser: vi.fn().mockResolvedValue(undefined),
      })
    ).rejects.toThrow();

    expect(credentials.saveCredentials).not.toHaveBeenCalled();
  });
});
