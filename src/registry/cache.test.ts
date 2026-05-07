import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { readCache, writeCache, cachePath, type CacheFile } from './cache.js';
import type { EventTypeSpec } from './client.js';

const SAMPLE_TYPE: EventTypeSpec = {
  id: 'engineering.deploy.completed.v1',
  domain: 'engineering',
  entity: 'deploy',
  action: 'completed',
  version: 1,
  schema: { type: 'object', properties: {} },
};

const SAMPLE_FILE: CacheFile = {
  version: 1,
  etag: 'W/"abc123"',
  fetchedAt: '2026-04-27T10:23:00Z',
  serverUrl: 'https://api.example.com',
  types: [SAMPLE_TYPE],
};

let tempHome: string;
let tempCwd: string;
let originalCwd: () => string;

beforeEach(() => {
  tempHome = mkdtempSync(join(tmpdir(), 'no-tickets-cache-home-'));
  tempCwd = mkdtempSync(join(tmpdir(), 'no-tickets-cache-cwd-'));
  vi.stubEnv('HOME', tempHome);
  vi.stubEnv('USERPROFILE', tempHome);
  vi.stubEnv('NO_TICKETS_HOME', tempHome);
  originalCwd = process.cwd;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (process as any).cwd = (): string => tempCwd;
});

afterEach(() => {
  vi.unstubAllEnvs();
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (process as any).cwd = originalCwd;
  rmSync(tempHome, { recursive: true, force: true });
  rmSync(tempCwd, { recursive: true, force: true });
});

describe('cachePath', () => {
  it('returns a user-local path when no .notickets/ ancestor exists', () => {
    const p = cachePath('https://api.example.com');

    expect(p.startsWith(tempHome)).toBe(true);
    expect(p).toContain('.notickets');
    expect(p).toContain('.cache');
    expect(p).toMatch(/registry-[0-9a-f]+\.json$/);
  });

  it('prefers a project-local .notickets/ in the cwd', () => {
    mkdirSync(join(tempCwd, '.notickets'));

    const p = cachePath('https://api.example.com');

    expect(p.startsWith(tempCwd)).toBe(true);
    expect(p).toContain(join('.notickets', '.cache'));
  });

  it('walks up to find an ancestor .notickets/ directory', () => {
    mkdirSync(join(tempCwd, '.notickets'));
    const child = join(tempCwd, 'sub', 'nested');
    mkdirSync(child, { recursive: true });
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (process as any).cwd = (): string => child;

    const p = cachePath('https://api.example.com');

    expect(p.startsWith(tempCwd)).toBe(true);
    expect(p).not.toContain(join('sub', 'nested'));
  });

  it('produces a different path per server URL (multi-server isolation)', () => {
    const a = cachePath('https://a.example.com');
    const b = cachePath('https://b.example.com');

    expect(a).not.toBe(b);
  });

  it('produces the same path for the same server URL across calls', () => {
    expect(cachePath('https://api.example.com')).toBe(cachePath('https://api.example.com'));
  });
});

describe('readCache', () => {
  it('returns null when the cache file does not exist', () => {
    expect(readCache('https://api.example.com')).toBeNull();
  });

  it('returns the parsed CacheFile when the file is valid', () => {
    const p = cachePath('https://api.example.com');
    mkdirSync(join(p, '..'), { recursive: true });
    writeFileSync(p, JSON.stringify(SAMPLE_FILE));

    expect(readCache('https://api.example.com')).toEqual(SAMPLE_FILE);
  });

  it('returns null when the cache file is not valid JSON (corrupt)', () => {
    const p = cachePath('https://api.example.com');
    mkdirSync(join(p, '..'), { recursive: true });
    writeFileSync(p, '{not json');

    expect(readCache('https://api.example.com')).toBeNull();
  });

  it('returns null when the cache file is JSON but the wrong shape', () => {
    const p = cachePath('https://api.example.com');
    mkdirSync(join(p, '..'), { recursive: true });
    writeFileSync(p, JSON.stringify({ etag: 'x', types: [] })); // missing version

    expect(readCache('https://api.example.com')).toBeNull();
  });

  it('returns null when the cache file has an unknown version', () => {
    const p = cachePath('https://api.example.com');
    mkdirSync(join(p, '..'), { recursive: true });
    writeFileSync(p, JSON.stringify({ ...SAMPLE_FILE, version: 99 }));

    expect(readCache('https://api.example.com')).toBeNull();
  });
});

describe('writeCache', () => {
  it('writes a valid CacheFile that readCache can round-trip', () => {
    writeCache('https://api.example.com', SAMPLE_FILE);

    expect(readCache('https://api.example.com')).toEqual(SAMPLE_FILE);
  });

  it('creates intermediate directories if they do not exist', () => {
    writeCache('https://api.example.com', SAMPLE_FILE);

    const p = cachePath('https://api.example.com');
    expect(existsSync(p)).toBe(true);
  });

  it('writes atomically: no temp file remains, final file has the new content', () => {
    writeCache('https://api.example.com', SAMPLE_FILE);
    const updated: CacheFile = { ...SAMPLE_FILE, etag: 'W/"new"' };
    writeCache('https://api.example.com', updated);

    const p = cachePath('https://api.example.com');
    const dir = join(p, '..');
    const survivors = require('node:fs').readdirSync(dir) as string[];

    expect(survivors).toHaveLength(1);
    expect(JSON.parse(readFileSync(p, 'utf-8'))).toEqual(updated);
  });

  it('rejects a CacheFile with an unknown version (defensive guard against caller bugs)', () => {
    const bad = { ...SAMPLE_FILE, version: 99 } as unknown as CacheFile;

    expect(() => writeCache('https://api.example.com', bad)).toThrow();
  });

  it('isolates writes per server URL', () => {
    const a: CacheFile = { ...SAMPLE_FILE, etag: 'a', serverUrl: 'https://a.example.com' };
    const b: CacheFile = { ...SAMPLE_FILE, etag: 'b', serverUrl: 'https://b.example.com' };

    writeCache('https://a.example.com', a);
    writeCache('https://b.example.com', b);

    expect(readCache('https://a.example.com')?.etag).toBe('a');
    expect(readCache('https://b.example.com')?.etag).toBe('b');
  });
});
