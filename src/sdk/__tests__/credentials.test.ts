import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  saveCredentials,
  loadCredentials,
  clearCredentials,
  type StoredCredentials,
} from '../credentials.js';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';

vi.mock('node:fs');
vi.mock('node:os');

const CREDENTIALS_DIR = '/home/testuser/.notickets';
const CREDENTIALS_PATH = path.join(CREDENTIALS_DIR, 'credentials');

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(os.homedir).mockReturnValue('/home/testuser');
  vi.mocked(os.platform).mockReturnValue('linux');
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('saveCredentials', () => {
  it('creates the .notickets directory if it does not exist', () => {
    vi.mocked(fs.existsSync).mockImplementation((p) => p !== CREDENTIALS_DIR);
    vi.mocked(fs.writeFileSync).mockReturnValue(undefined);
    vi.mocked(fs.mkdirSync).mockReturnValue(undefined);
    vi.mocked(fs.chmodSync).mockReturnValue(undefined);

    saveCredentials('nt_session_abc123', 'user@example.com', '2026-05-01T00:00:00Z');

    expect(fs.mkdirSync).toHaveBeenCalledWith(CREDENTIALS_DIR, { recursive: true, mode: 0o700 });
  });

  it('skips mkdir when directory already exists', () => {
    vi.mocked(fs.existsSync).mockReturnValue(true);
    vi.mocked(fs.writeFileSync).mockReturnValue(undefined);
    vi.mocked(fs.chmodSync).mockReturnValue(undefined);

    saveCredentials('nt_session_abc123', 'user@example.com', '2026-05-01T00:00:00Z');

    expect(fs.mkdirSync).not.toHaveBeenCalled();
  });

  it('writes credentials as JSON to ~/.notickets/credentials', () => {
    vi.mocked(fs.existsSync).mockReturnValue(true);
    vi.mocked(fs.writeFileSync).mockReturnValue(undefined);
    vi.mocked(fs.chmodSync).mockReturnValue(undefined);

    saveCredentials('nt_session_abc123', 'user@example.com', '2026-05-01T00:00:00Z');

    expect(fs.writeFileSync).toHaveBeenCalledOnce();
    const [filePath, content] = vi.mocked(fs.writeFileSync).mock.calls[0]!;
    expect(filePath).toBe(CREDENTIALS_PATH);
    expect(typeof content).toBe('string');

    const parsed = JSON.parse(String(content)) as StoredCredentials;
    expect(parsed.token).toBe('nt_session_abc123');
    expect(parsed.email).toBe('user@example.com');
    expect(parsed.expiresAt).toBe('2026-05-01T00:00:00Z');
    expect(vi.mocked(fs.writeFileSync).mock.calls[0]![2]).toBe('utf-8');
  });

  it('sets file permissions to 600 on POSIX systems', () => {
    vi.mocked(fs.existsSync).mockReturnValue(true);
    vi.mocked(fs.writeFileSync).mockReturnValue(undefined);
    vi.mocked(fs.chmodSync).mockReturnValue(undefined);

    saveCredentials('nt_session_abc123', 'user@example.com', '2026-05-01T00:00:00Z');

    expect(fs.chmodSync).toHaveBeenCalledWith(CREDENTIALS_PATH, 0o600);
  });

  it('skips chmod on Windows', () => {
    vi.mocked(os.platform).mockReturnValue('win32');
    vi.mocked(fs.existsSync).mockReturnValue(true);
    vi.mocked(fs.writeFileSync).mockReturnValue(undefined);

    saveCredentials('nt_session_abc123', 'user@example.com', '2026-05-01T00:00:00Z');

    expect(fs.chmodSync).not.toHaveBeenCalled();
  });
});

describe('loadCredentials', () => {
  it('returns credentials when file exists and token is not expired', () => {
    const stored: StoredCredentials = {
      token: 'nt_session_abc123',
      email: 'user@example.com',
      expiresAt: '2099-01-01T00:00:00Z',
    };
    vi.mocked(fs.existsSync).mockReturnValue(true);
    vi.mocked(fs.readFileSync).mockReturnValue(JSON.stringify(stored));

    const result = loadCredentials();

    expect(result).toEqual(stored);
    expect(fs.readFileSync).toHaveBeenCalledWith(CREDENTIALS_PATH, 'utf-8');
  });

  it('returns null when credentials file does not exist', () => {
    vi.mocked(fs.existsSync).mockReturnValue(false);

    const result = loadCredentials();

    expect(result).toBeNull();
  });

  it('returns null when token is expired', () => {
    const stored: StoredCredentials = {
      token: 'nt_session_abc123',
      email: 'user@example.com',
      expiresAt: '2020-01-01T00:00:00Z',
    };
    vi.mocked(fs.existsSync).mockReturnValue(true);
    vi.mocked(fs.readFileSync).mockReturnValue(JSON.stringify(stored));

    const result = loadCredentials();

    expect(result).toBeNull();
  });

  it('returns null when file contains invalid JSON', () => {
    vi.mocked(fs.existsSync).mockReturnValue(true);
    vi.mocked(fs.readFileSync).mockReturnValue('not-json');

    const result = loadCredentials();

    expect(result).toBeNull();
  });

  it('returns null when token expires at exactly the current time', () => {
    try {
      vi.useFakeTimers();
      vi.setSystemTime(new Date('2026-06-01T12:00:00Z'));

      const stored: StoredCredentials = {
        token: 'nt_session_abc123',
        email: 'user@example.com',
        expiresAt: '2026-06-01T12:00:00Z',
      };
      vi.mocked(fs.existsSync).mockReturnValue(true);
      vi.mocked(fs.readFileSync).mockReturnValue(JSON.stringify(stored));

      const result = loadCredentials();

      expect(result).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it('returns null when file is missing required fields', () => {
    vi.mocked(fs.existsSync).mockReturnValue(true);
    vi.mocked(fs.readFileSync).mockReturnValue(JSON.stringify({ token: 'abc' }));

    const result = loadCredentials();

    expect(result).toBeNull();
  });

  it('returns null when required fields are present but not strings', () => {
    vi.mocked(fs.existsSync).mockReturnValue(true);
    vi.mocked(fs.readFileSync).mockReturnValue(
      JSON.stringify({ token: 123, email: true, expiresAt: null })
    );

    expect(loadCredentials()).toBeNull();
  });

  it('returns null when parsed JSON is a primitive', () => {
    vi.mocked(fs.existsSync).mockReturnValue(true);
    vi.mocked(fs.readFileSync).mockReturnValue('42');

    expect(loadCredentials()).toBeNull();
  });

  it('returns null when parsed JSON is an array', () => {
    vi.mocked(fs.existsSync).mockReturnValue(true);
    vi.mocked(fs.readFileSync).mockReturnValue('[]');

    expect(loadCredentials()).toBeNull();
  });

  it('returns null when token is missing but email and expiresAt are present', () => {
    vi.mocked(fs.existsSync).mockReturnValue(true);
    vi.mocked(fs.readFileSync).mockReturnValue(
      JSON.stringify({ email: 'a@b.com', expiresAt: '2099-01-01T00:00:00Z' })
    );

    expect(loadCredentials()).toBeNull();
  });

  it('returns null when expiresAt is missing but token and email are present', () => {
    vi.mocked(fs.existsSync).mockReturnValue(true);
    vi.mocked(fs.readFileSync).mockReturnValue(
      JSON.stringify({ token: 'nt_session_abc', email: 'a@b.com' })
    );

    expect(loadCredentials()).toBeNull();
  });
});

describe('clearCredentials', () => {
  it('deletes the credentials file when it exists', () => {
    vi.mocked(fs.existsSync).mockReturnValue(true);
    vi.mocked(fs.unlinkSync).mockReturnValue(undefined);

    clearCredentials();

    expect(fs.unlinkSync).toHaveBeenCalledWith(CREDENTIALS_PATH);
  });

  it('does nothing when credentials file does not exist', () => {
    vi.mocked(fs.existsSync).mockReturnValue(false);

    clearCredentials();

    expect(fs.unlinkSync).not.toHaveBeenCalled();
  });
});
