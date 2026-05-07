import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  readFileSync,
  readdirSync,
  rmSync,
  existsSync,
  statSync,
} from 'node:fs';
import { join, sep } from 'node:path';
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
let cwdSpy: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  tempHome = mkdtempSync(join(tmpdir(), 'no-tickets-cache-home-'));
  tempCwd = mkdtempSync(join(tmpdir(), 'no-tickets-cache-cwd-'));
  // Stub HOME / USERPROFILE so os.homedir() resolves to the temp dir.
  vi.stubEnv('HOME', tempHome);
  vi.stubEnv('USERPROFILE', tempHome);
  // NO_TICKETS_HOME is the explicit override path; default tests must
  // exercise the homedir() branch, so leave it unset.
  delete process.env['NO_TICKETS_HOME'];
  cwdSpy = vi.spyOn(process, 'cwd').mockReturnValue(tempCwd);
});

afterEach(() => {
  cwdSpy.mockRestore();
  vi.unstubAllEnvs();
  rmSync(tempHome, { recursive: true, force: true });
  rmSync(tempCwd, { recursive: true, force: true });
});

const NOTICKETS_PATH_FRAGMENT = `${sep}.notickets${sep}`;

describe('cachePath', () => {
  it('returns a user-local path under homedir() when no .notickets/ ancestor exists', () => {
    const p = cachePath('https://api.example.com');

    expect(p.startsWith(tempHome + sep)).toBe(true);
    expect(p).toContain(`${NOTICKETS_PATH_FRAGMENT}.cache${sep}`);
    expect(p).toMatch(/registry-[0-9a-f]{16}\.json$/);
  });

  it('respects NO_TICKETS_HOME when set, falling back away from homedir()', () => {
    const altHome = mkdtempSync(join(tmpdir(), 'no-tickets-alt-'));
    try {
      process.env['NO_TICKETS_HOME'] = altHome;
      const p = cachePath('https://api.example.com');

      expect(p.startsWith(altHome + sep)).toBe(true);
      expect(p.startsWith(tempHome + sep)).toBe(false);
    } finally {
      delete process.env['NO_TICKETS_HOME'];
      rmSync(altHome, { recursive: true, force: true });
    }
  });

  it('prefers a project-local .notickets/ in the cwd', () => {
    mkdirSync(join(tempCwd, '.notickets'));

    const p = cachePath('https://api.example.com');

    expect(p.startsWith(tempCwd + sep)).toBe(true);
    expect(p).toContain(`${NOTICKETS_PATH_FRAGMENT}.cache${sep}`);
  });

  it('walks up to find an ancestor .notickets/ directory', () => {
    mkdirSync(join(tempCwd, '.notickets'));
    const child = join(tempCwd, 'sub', 'nested');
    mkdirSync(child, { recursive: true });
    cwdSpy.mockReturnValue(child);

    const p = cachePath('https://api.example.com');

    expect(p.startsWith(tempCwd + sep)).toBe(true);
    expect(p.startsWith(child + sep)).toBe(false);
  });

  it('stops the ancestor walk at git root (does not capture an unrelated ~/.notickets/)', () => {
    // Layout: tempCwd is the "git repo" (.git is here); cwd is a subdir;
    // there's NO .notickets anywhere in the repo. Walk should hit .git and
    // bail out, NOT walk all the way up to find the user-home .notickets.
    mkdirSync(join(tempHome, '.notickets')); // unrelated ancestor we must NOT pick
    mkdirSync(join(tempCwd, '.git'));
    const child = join(tempCwd, 'sub');
    mkdirSync(child);
    cwdSpy.mockReturnValue(child);

    const p = cachePath('https://api.example.com');

    // Falls back to user-local under homedir, not project-local under tempCwd.
    expect(p.startsWith(tempHome + sep)).toBe(true);
  });

  it('treats `.git` as a hard wall — walks past `.git` does NOT pick up a higher .notickets', () => {
    // Layout:
    //   tempCwd/.notickets/         ← MUST NOT be picked
    //   tempCwd/project/.git/
    //   tempCwd/project/sub/        ← cwd
    //
    // With the .git guard active, findAncestorNoticketsDir stops at
    // tempCwd/project and returns null → cachePath uses homeBase().
    // Without the guard, it walks up past .git and finds tempCwd/.notickets,
    // resolving to tempCwd/.notickets/.cache/...
    mkdirSync(join(tempCwd, '.notickets'));
    const project = join(tempCwd, 'project');
    mkdirSync(join(project, '.git'), { recursive: true });
    const cwd = join(project, 'sub');
    mkdirSync(cwd);
    cwdSpy.mockReturnValue(cwd);

    const p = cachePath('https://api.example.com');

    expect(p.startsWith(tempCwd + sep + '.notickets')).toBe(false);
    expect(p.startsWith(tempHome + sep)).toBe(true);
  });

  it('ignores a regular file named ".notickets" (only directories count)', () => {
    writeFileSync(join(tempCwd, '.notickets'), 'not a directory');

    const p = cachePath('https://api.example.com');

    // Ancestor lookup should skip the file and fall back to user-local.
    expect(p.startsWith(tempHome + sep)).toBe(true);
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

  it('leaves no orphan temp files in the cache directory after successive writes', () => {
    writeCache('https://api.example.com', SAMPLE_FILE);
    const updated: CacheFile = { ...SAMPLE_FILE, etag: 'W/"new"' };
    writeCache('https://api.example.com', updated);

    const p = cachePath('https://api.example.com');
    const dir = join(p, '..');
    const survivors = readdirSync(dir);

    expect(survivors).toHaveLength(1);
    expect(survivors[0]).not.toMatch(/\.tmp$/);
    expect(JSON.parse(readFileSync(p, 'utf-8'))).toEqual(updated);
  });

  it.skipIf(process.platform === 'win32')(
    'writes the cache file with mode 0o600 (owner read/write only)',
    () => {
      writeCache('https://api.example.com', SAMPLE_FILE);

      const p = cachePath('https://api.example.com');
      const mode = statSync(p).mode & 0o777;
      expect(mode).toBe(0o600);
    },
  );

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
