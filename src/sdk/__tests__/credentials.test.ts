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
  vi.mocked(os.homedir).mockReturnValue('/home/testuser');
  vi.mocked(os.platform).mockReturnValue('linux');
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('saveCredentials', () => {
  it('creates the .notickets directory if it does not exist', () => {
    vi.mocked(fs.existsSync).mockReturnValue(false);
    vi.mocked(fs.writeFileSync).mockReturnValue(undefined);
    vi.mocked(fs.mkdirSync).mockReturnValue(undefined);
    vi.mocked(fs.chmodSync).mockReturnValue(undefined);

    saveCredentials('nt_session_abc123', 'user@example.com', '2026-05-01T00:00:00Z');

    expect(fs.mkdirSync).toHaveBeenCalledWith(CREDENTIALS_DIR, { recursive: true });
  });

  it('writes credentials as JSON to ~/.notickets/credentials', () => {
    vi.mocked(fs.existsSync).mockReturnValue(true);
    vi.mocked(fs.writeFileSync).mockReturnValue(undefined);
    vi.mocked(fs.chmodSync).mockReturnValue(undefined);

    saveCredentials('nt_session_abc123', 'user@example.com', '2026-05-01T00:00:00Z');

    expect(fs.writeFileSync).toHaveBeenCalledOnce();
    const [filePath, content] = vi.mocked(fs.writeFileSync).mock.calls[0]!;
    expect(filePath).toBe(CREDENTIALS_PATH);

    const parsed = JSON.parse(content as string) as StoredCredentials;
    expect(parsed.token).toBe('nt_session_abc123');
    expect(parsed.email).toBe('user@example.com');
    expect(parsed.expiresAt).toBe('2026-05-01T00:00:00Z');
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

  it('returns null when file is missing required fields', () => {
    vi.mocked(fs.existsSync).mockReturnValue(true);
    vi.mocked(fs.readFileSync).mockReturnValue(JSON.stringify({ token: 'abc' }));

    const result = loadCredentials();

    expect(result).toBeNull();
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
