import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { buildPushAuth } from '../push-auth.js';
import * as auth from '../../sdk/auth.js';

vi.mock('../../sdk/auth.js');

beforeEach(() => {
  vi.clearAllMocks();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('buildPushAuth', () => {
  it('uses session token with teamId and projectId from local config', () => {
    vi.mocked(auth.resolveAuth).mockReturnValue({
      token: 'nt_session_abc123',
      source: 'credentials',
      tokenType: 'session',
    });

    const result = buildPushAuth({
      apiUrl: 'https://api.no-tickets.com',
      teamId: 'team-abc',
      projectId: 'proj-xyz',
    });

    expect(result).toEqual({
      token: 'nt_session_abc123',
      apiUrl: 'https://api.no-tickets.com',
      teamId: 'team-abc',
      projectId: 'proj-xyz',
      tokenType: 'session',
    });
    expect(result).not.toHaveProperty('source');
  });

  it('uses push token and omits teamId and projectId', () => {
    vi.mocked(auth.resolveAuth).mockReturnValue({
      token: 'nt_push_ci_token',
      source: 'env',
      tokenType: 'push',
    });

    const result = buildPushAuth({
      apiUrl: 'https://api.no-tickets.com',
      teamId: 'team-abc',
      projectId: 'proj-xyz',
    });

    expect(result).toEqual({
      token: 'nt_push_ci_token',
      apiUrl: 'https://api.no-tickets.com',
      teamId: undefined,
      projectId: undefined,
      tokenType: 'push',
    });
  });

  it('throws when resolveAuth throws (no auth available)', () => {
    vi.mocked(auth.resolveAuth).mockImplementation(() => {
      throw new Error('Not authenticated. Run `npx no-tickets init` to authenticate');
    });

    expect(() =>
      buildPushAuth({
        apiUrl: 'https://api.no-tickets.com',
        teamId: 'team-abc',
        projectId: 'proj-xyz',
      })
    ).toThrow('Not authenticated');
  });

  it('handles unknown token type same as push (omits teamId/projectId)', () => {
    vi.mocked(auth.resolveAuth).mockReturnValue({
      token: 'some_other_token',
      source: 'env',
      tokenType: 'unknown',
    });

    const result = buildPushAuth({
      apiUrl: 'https://api.no-tickets.com',
      teamId: 'team-abc',
      projectId: 'proj-xyz',
    });

    expect(result).toEqual({
      token: 'some_other_token',
      apiUrl: 'https://api.no-tickets.com',
      teamId: undefined,
      projectId: undefined,
      tokenType: 'unknown',
    });
  });
});
