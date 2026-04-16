import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { resolveAuth } from '../auth.js';
import * as credentials from '../credentials.js';

vi.mock('../credentials.js');

beforeEach(() => {
  vi.clearAllMocks();
  vi.stubEnv('NO_TICKETS_TOKEN', '');
});

afterEach(() => {
  vi.unstubAllEnvs();
});

describe('resolveAuth', () => {
  it('returns push token from NO_TICKETS_TOKEN env var', () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_push_abc123');

    const result = resolveAuth();

    expect(result).toEqual({
      token: 'nt_push_abc123',
      source: 'env',
      tokenType: 'push',
    });
  });

  it('returns session token from credentials file when env var is not set', () => {
    vi.stubEnv('NO_TICKETS_TOKEN', '');
    vi.mocked(credentials.loadCredentials).mockReturnValue({
      token: 'nt_session_xyz789',
      email: 'user@example.com',
      expiresAt: '2099-01-01T00:00:00Z',
    });

    const result = resolveAuth();

    expect(result).toEqual({
      token: 'nt_session_xyz789',
      source: 'credentials',
      tokenType: 'session',
    });
  });

  it('prefers env var over credentials file', () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_push_envtoken');
    vi.mocked(credentials.loadCredentials).mockReturnValue({
      token: 'nt_session_filetoken',
      email: 'user@example.com',
      expiresAt: '2099-01-01T00:00:00Z',
    });

    const result = resolveAuth();

    expect(result.token).toBe('nt_push_envtoken');
    expect(result.source).toBe('env');
    expect(credentials.loadCredentials).not.toHaveBeenCalled();
  });

  it('throws when neither env var nor credentials are available', () => {
    vi.stubEnv('NO_TICKETS_TOKEN', '');
    vi.mocked(credentials.loadCredentials).mockReturnValue(null);

    expect(() => resolveAuth()).toThrow(
      'Not authenticated. Run `npx no-tickets init` to authenticate'
    );
  });

  it('identifies push token type from nt_push_ prefix', () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_push_something');

    const result = resolveAuth();

    expect(result.tokenType).toBe('push');
  });

  it('identifies session token type from nt_session_ prefix', () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_session_something');

    const result = resolveAuth();

    expect(result.tokenType).toBe('session');
  });

  it('defaults to unknown token type for unrecognized prefix', () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'some_other_token');

    const result = resolveAuth();

    expect(result.tokenType).toBe('unknown');
  });

  it('treats undefined env var same as empty', () => {
    delete process.env['NO_TICKETS_TOKEN'];
    vi.mocked(credentials.loadCredentials).mockReturnValue({
      token: 'nt_session_fromfile',
      email: 'user@example.com',
      expiresAt: '2099-01-01T00:00:00Z',
    });

    const result = resolveAuth();

    expect(result.source).toBe('credentials');
  });
});
