import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { resolveAuth } from '../sdk/auth.js';
import { saveCredentials, loadCredentials, clearCredentials } from '../sdk/credentials.js';

let testDir: string;

beforeEach(async () => {
  testDir = await mkdtemp(join(tmpdir(), 'nt-auth-e2e-'));
  vi.stubEnv('HOME', testDir);
  vi.stubEnv('NO_TICKETS_TOKEN', '');
  delete process.env['NO_TICKETS_TOKEN'];
});

afterEach(async () => {
  vi.unstubAllEnvs();
  await rm(testDir, { recursive: true, force: true });
});

describe('auth resolution e2e', () => {
  it('resolves auth from NO_TICKETS_TOKEN env var', () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_push_envtoken');

    const auth = resolveAuth();

    expect(auth.token).toBe('nt_push_envtoken');
    expect(auth.source).toBe('env');
    expect(auth.tokenType).toBe('push');
  });

  it('detects session token type', () => {
    vi.stubEnv('NO_TICKETS_TOKEN', 'nt_session_abc');

    const auth = resolveAuth();

    expect(auth.tokenType).toBe('session');
  });

  it('throws when neither env var nor credentials exist', () => {
    expect(() => resolveAuth()).toThrow('Not authenticated');
  });
});

describe('credential storage e2e', () => {
  it('saves and loads credentials round-trip', () => {
    const futureDate = new Date(Date.now() + 3600_000).toISOString();
    saveCredentials('nt_session_test', 'user@test.com', futureDate);

    const loaded = loadCredentials();

    expect(loaded).not.toBeNull();
    expect(loaded?.token).toBe('nt_session_test');
  });

  it('returns null for expired credentials', () => {
    const pastDate = new Date(Date.now() - 1000).toISOString();
    saveCredentials('nt_session_expired', 'user@test.com', pastDate);

    const loaded = loadCredentials();

    expect(loaded).toBeNull();
  });

  it('clears credentials', () => {
    const futureDate = new Date(Date.now() + 3600_000).toISOString();
    saveCredentials('nt_session_clear', 'user@test.com', futureDate);
    clearCredentials();

    const loaded = loadCredentials();

    expect(loaded).toBeNull();
  });
});
