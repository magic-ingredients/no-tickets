import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { readNoTicketsDir } from '../fs.js';

let testDir: string;
let originalCwd: string;

beforeEach(async () => {
  testDir = await mkdtemp(join(tmpdir(), 'nt-fs-test-'));
  originalCwd = process.cwd();
  process.chdir(testDir);
});

afterEach(async () => {
  process.chdir(originalCwd);
  await rm(testDir, { recursive: true, force: true });
});

describe('readNoTicketsDir', () => {
  it('reads .md files from top-level directory', async () => {
    await mkdir('.notickets');
    await writeFile('.notickets/epic.md', '# Epic');

    const files = await readNoTicketsDir('.notickets');

    expect(files).toHaveLength(1);
    expect(files[0]?.content).toBe('# Epic');
    expect(files[0]?.path).toContain('epic.md');
  });

  it('reads .md files from subdirectories', async () => {
    await mkdir('.notickets/auth', { recursive: true });
    await writeFile('.notickets/auth/epic.md', '# Auth Epic');
    await writeFile('.notickets/auth/login.md', '# Login');

    const files = await readNoTicketsDir('.notickets');

    expect(files).toHaveLength(2);
    const names = files.map((f) => f.path);
    expect(names.some((n) => n.includes('epic.md'))).toBe(true);
    expect(names.some((n) => n.includes('login.md'))).toBe(true);
  });

  it('ignores non-.md files', async () => {
    await mkdir('.notickets');
    await writeFile('.notickets/readme.txt', 'not markdown');
    await writeFile('.notickets/epic.md', '# Epic');

    const files = await readNoTicketsDir('.notickets');

    expect(files).toHaveLength(1);
    expect(files[0]?.path).toContain('epic.md');
  });

  it('returns empty array for non-existent directory', async () => {
    const files = await readNoTicketsDir('.notickets');

    expect(files).toEqual([]);
  });

  it('returns empty array for empty directory', async () => {
    await mkdir('.notickets');

    const files = await readNoTicketsDir('.notickets');

    expect(files).toEqual([]);
  });

  it('rejects absolute paths outside cwd', async () => {
    await expect(readNoTicketsDir('/etc')).rejects.toThrow('outside');
  });

  it('rejects paths with .. traversal', async () => {
    await expect(readNoTicketsDir('.notickets/../../etc')).rejects.toThrow('outside');
  });

  it('accepts relative paths within cwd', async () => {
    await mkdir('.notickets');
    await writeFile('.notickets/epic.md', '# Epic');

    const files = await readNoTicketsDir('.notickets');

    expect(files).toHaveLength(1);
  });
});
