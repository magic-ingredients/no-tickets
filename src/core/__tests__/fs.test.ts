import { describe, it, expect, afterEach } from 'vitest';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { readNoTicketsDir } from '../fs.js';

let testDir: string;

afterEach(async () => {
  if (testDir) {
    await rm(testDir, { recursive: true, force: true });
  }
});

async function setupDir() {
  testDir = await mkdtemp(join(tmpdir(), 'nt-fs-test-'));
  return testDir;
}

describe('readNoTicketsDir', () => {
  it('reads .md files from top-level directory', async () => {
    const dir = await setupDir();
    await writeFile(join(dir, 'epic.md'), '# Epic');

    const files = await readNoTicketsDir(dir);

    expect(files).toHaveLength(1);
    expect(files[0]?.content).toBe('# Epic');
    expect(files[0]?.path).toContain('epic.md');
  });

  it('reads .md files from subdirectories', async () => {
    const dir = await setupDir();
    await mkdir(join(dir, 'auth'));
    await writeFile(join(dir, 'auth', 'epic.md'), '# Auth Epic');
    await writeFile(join(dir, 'auth', 'login.md'), '# Login');

    const files = await readNoTicketsDir(dir);

    expect(files).toHaveLength(2);
    const names = files.map((f) => f.path);
    expect(names.some((n) => n.includes('epic.md'))).toBe(true);
    expect(names.some((n) => n.includes('login.md'))).toBe(true);
  });

  it('ignores non-.md files', async () => {
    const dir = await setupDir();
    await writeFile(join(dir, 'readme.txt'), 'not markdown');
    await writeFile(join(dir, 'epic.md'), '# Epic');

    const files = await readNoTicketsDir(dir);

    expect(files).toHaveLength(1);
    expect(files[0]?.path).toContain('epic.md');
  });

  it('returns empty array for non-existent directory', async () => {
    const files = await readNoTicketsDir('/tmp/does-not-exist-' + Date.now());

    expect(files).toEqual([]);
  });

  it('returns empty array for empty directory', async () => {
    const dir = await setupDir();

    const files = await readNoTicketsDir(dir);

    expect(files).toEqual([]);
  });

  it('rejects paths outside current working directory', async () => {
    await expect(readNoTicketsDir('/etc')).rejects.toThrow();
  });

  it('rejects paths with .. traversal', async () => {
    await expect(readNoTicketsDir('.notickets/../../etc')).rejects.toThrow();
  });

  it('accepts relative paths within cwd', async () => {
    const dir = await setupDir();
    const originalCwd = process.cwd();
    process.chdir(dir);

    await mkdir(join(dir, '.notickets'));
    await writeFile(join(dir, '.notickets', 'epic.md'), '# Epic');

    const files = await readNoTicketsDir('.notickets');

    expect(files).toHaveLength(1);
    process.chdir(originalCwd);
  });
});
