import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { resolveUrls, loadProfile, DEFAULT_AUTH_URL } from '../url-resolver.js';
import { DEFAULT_API_URL } from '../auth.js';

let testDir: string;

beforeEach(async () => {
  testDir = await mkdtemp(join(tmpdir(), 'nt-url-resolver-'));
  vi.stubEnv('NO_TICKETS_HOME', testDir);
  delete process.env['NO_TICKETS_API_URL'];
  delete process.env['NO_TICKETS_AUTH_URL'];
});

afterEach(async () => {
  vi.unstubAllEnvs();
  await rm(testDir, { recursive: true, force: true });
});

async function writeConfig(content: string): Promise<void> {
  await mkdir(join(testDir, '.notickets'), { recursive: true });
  await writeFile(join(testDir, '.notickets', 'config.json'), content);
}

describe('resolveUrls', () => {
  it('returns production defaults when nothing is set', () => {
    const resolved = resolveUrls({});
    expect(resolved).toEqual({
      apiUrl: DEFAULT_API_URL,
      authUrl: DEFAULT_AUTH_URL,
      source: 'default',
    });
  });

  it('returns env vars when both NO_TICKETS_API_URL and NO_TICKETS_AUTH_URL are set', () => {
    vi.stubEnv('NO_TICKETS_API_URL', 'https://api-staging.example.com');
    vi.stubEnv('NO_TICKETS_AUTH_URL', 'https://app-staging.example.com/api/auth/cli');

    const resolved = resolveUrls({});
    expect(resolved).toEqual({
      apiUrl: 'https://api-staging.example.com',
      authUrl: 'https://app-staging.example.com/api/auth/cli',
      source: 'env',
    });
  });

  it('throws when only NO_TICKETS_API_URL is set (pair validation)', () => {
    vi.stubEnv('NO_TICKETS_API_URL', 'https://api.example.com');

    expect(() => resolveUrls({})).toThrow(/NO_TICKETS_AUTH_URL is not/);
  });

  it('throws when only NO_TICKETS_AUTH_URL is set (pair validation)', () => {
    vi.stubEnv('NO_TICKETS_AUTH_URL', 'https://app.example.com/api/auth/cli');

    expect(() => resolveUrls({})).toThrow(/NO_TICKETS_API_URL is not/);
  });

  it('pair-validation error includes the offending value', () => {
    vi.stubEnv('NO_TICKETS_API_URL', 'https://typo.example.com');

    expect(() => resolveUrls({})).toThrow(/typo\.example\.com/);
  });

  it('treats whitespace-only env vars as unset', () => {
    vi.stubEnv('NO_TICKETS_API_URL', '   ');
    vi.stubEnv('NO_TICKETS_AUTH_URL', '\t');

    const resolved = resolveUrls({});
    expect(resolved.source).toBe('default');
  });

  it('treats empty-string env vars as unset', () => {
    vi.stubEnv('NO_TICKETS_API_URL', '');
    vi.stubEnv('NO_TICKETS_AUTH_URL', '');

    const resolved = resolveUrls({});
    expect(resolved.source).toBe('default');
  });
});

describe('resolveUrls with --profile', () => {
  it('loads the named profile from ~/.notickets/config.json', async () => {
    await writeConfig(JSON.stringify({
      profiles: {
        staging: {
          apiUrl: 'https://api-staging.example.com',
          authUrl: 'https://app-staging.example.com/api/auth/cli',
        },
      },
    }));

    const resolved = resolveUrls({ profile: 'staging' });
    expect(resolved).toEqual({
      apiUrl: 'https://api-staging.example.com',
      authUrl: 'https://app-staging.example.com/api/auth/cli',
      source: 'profile',
    });
  });

  it('errors when the config file does not exist', () => {
    expect(() => resolveUrls({ profile: 'staging' })).toThrow(/does not exist/);
  });

  it('error message when config missing includes a one-line example', () => {
    try {
      resolveUrls({ profile: 'staging' });
      throw new Error('expected throw');
    } catch (e) {
      expect((e as Error).message).toContain('"profiles"');
      expect((e as Error).message).toContain('"staging"');
    }
  });

  it('errors when the named profile is missing from the file', async () => {
    await writeConfig(JSON.stringify({ profiles: { production: { apiUrl: 'x', authUrl: 'y' } } }));

    expect(() => resolveUrls({ profile: 'staging' })).toThrow(/Available: production/);
  });

  it('errors with a distinct "invalid JSON" message when the file is malformed', async () => {
    await writeConfig('not json');

    expect(() => resolveUrls({ profile: 'staging' })).toThrow(/contains invalid JSON/);
  });

  it('errors when the profile entry is missing apiUrl', async () => {
    await writeConfig(JSON.stringify({ profiles: { staging: { authUrl: 'https://x' } } }));

    expect(() => resolveUrls({ profile: 'staging' })).toThrow(/is invalid/);
  });

  it('errors when apiUrl is not an http(s) URL', async () => {
    await writeConfig(JSON.stringify({
      profiles: { staging: { apiUrl: 'not-a-url', authUrl: 'https://x' } },
    }));

    expect(() => resolveUrls({ profile: 'staging' })).toThrow(/is invalid/);
  });

  it('errors when apiUrl is an empty string', async () => {
    await writeConfig(JSON.stringify({
      profiles: { staging: { apiUrl: '', authUrl: 'https://x' } },
    }));

    expect(() => resolveUrls({ profile: 'staging' })).toThrow(/is invalid/);
  });

  it('--profile wins over env vars', async () => {
    vi.stubEnv('NO_TICKETS_API_URL', 'https://from-env.example.com');
    vi.stubEnv('NO_TICKETS_AUTH_URL', 'https://from-env-auth.example.com/api/auth/cli');
    await writeConfig(JSON.stringify({
      profiles: { staging: { apiUrl: 'https://from-profile.example.com', authUrl: 'https://from-profile-auth.example.com/api/auth/cli' } },
    }));

    const resolved = resolveUrls({ profile: 'staging' });
    expect(resolved.source).toBe('profile');
    expect(resolved.apiUrl).toBe('https://from-profile.example.com');
  });
});

describe('loadProfile', () => {
  it('returns the profile object when present', async () => {
    await writeConfig(JSON.stringify({
      profiles: { staging: { apiUrl: 'https://a', authUrl: 'https://b' } },
    }));

    expect(loadProfile('staging')).toEqual({ apiUrl: 'https://a', authUrl: 'https://b' });
  });
});
