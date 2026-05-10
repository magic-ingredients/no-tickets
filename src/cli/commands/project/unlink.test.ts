import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mkdtemp, mkdir, writeFile, readFile, rm, stat } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runProjectUnlink } from './unlink.js';

let testDir: string;
let logSpy: ReturnType<typeof vi.spyOn>;
let errSpy: ReturnType<typeof vi.spyOn>;

beforeEach(async () => {
  testDir = await mkdtemp(join(tmpdir(), 'nt-project-unlink-'));
  vi.stubEnv('NO_TICKETS_HOME', testDir);
  process.exitCode = undefined;
  logSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
  errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
});

afterEach(async () => {
  vi.unstubAllEnvs();
  vi.restoreAllMocks();
  await rm(testDir, { recursive: true, force: true });
});

async function writeConfig(content: object): Promise<void> {
  await mkdir(join(testDir, '.notickets'), { recursive: true });
  await writeFile(join(testDir, '.notickets', 'config.json'), JSON.stringify(content));
}

async function readConfig(): Promise<Record<string, unknown>> {
  const raw = await readFile(join(testDir, '.notickets', 'config.json'), 'utf-8');
  return JSON.parse(raw) as Record<string, unknown>;
}

describe('runProjectUnlink', () => {
  it('removes the named entry while preserving the rest of the projects section', async () => {
    await writeConfig({
      profiles: { staging: { apiUrl: 'https://x', authUrl: 'https://x/auth' } },
      projects: {
        keep1: { profile: 'staging', pushToken: 'nt_push_k1' },
        remove: { profile: 'staging', pushToken: 'nt_push_r' },
        keep2: { profile: 'staging', pushToken: 'nt_push_k2' },
      },
    });

    const exit = await runProjectUnlink({ name: 'remove' });
    expect(exit).toBe(0);

    const config = await readConfig();
    expect(config['projects']).toEqual({
      keep1: { profile: 'staging', pushToken: 'nt_push_k1' },
      keep2: { profile: 'staging', pushToken: 'nt_push_k2' },
    });
  });

  it('preserves profiles section unchanged on unlink', async () => {
    const profiles = {
      staging: { apiUrl: 'https://x', authUrl: 'https://x/auth' },
      production: { apiUrl: 'https://y', authUrl: 'https://y/auth' },
    };
    await writeConfig({
      profiles,
      projects: { remove: { profile: 'staging', pushToken: 'nt_push_r' } },
    });

    await runProjectUnlink({ name: 'remove' });

    const config = await readConfig();
    expect(config['profiles']).toEqual(profiles);
  });

  it('preserves file mode 0600 after rewrite', async () => {
    await writeConfig({
      profiles: { staging: { apiUrl: 'https://x', authUrl: 'https://x/auth' } },
      projects: {
        keep: { profile: 'staging', pushToken: 'nt_push_k' },
        remove: { profile: 'staging', pushToken: 'nt_push_r' },
      },
    });

    await runProjectUnlink({ name: 'remove' });

    const stats = await stat(join(testDir, '.notickets', 'config.json'));
    expect(stats.mode & 0o777).toBe(0o600);
  });

  it('exits 1 with a helpful error when the named project is not registered', async () => {
    await writeConfig({
      profiles: {},
      projects: { keep: { profile: 'staging', pushToken: 'nt_push_k' } },
    });

    const exit = await runProjectUnlink({ name: 'no-such-project' });
    expect(exit).toBe(1);

    const errOutput = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(errOutput).toMatch(/no-such-project/);
    expect(errOutput).toMatch(/not registered|not linked/i);

    // Untouched
    const config = await readConfig();
    expect(config['projects']).toEqual({
      keep: { profile: 'staging', pushToken: 'nt_push_k' },
    });
  });

  it('exits 1 with the literal usage message when name is empty (pins the wording)', async () => {
    const exit = await runProjectUnlink({ name: '' });
    expect(exit).toBe(1);
    const err = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(err).toMatch(/<name> is required/);
  });

  it.each([
    ['null', null],
    ['array', []],
  ])('treats projects: %s as "no projects registered" (isRecord guard)', async (_label, value) => {
    await writeConfig({ profiles: {}, projects: value });

    const exit = await runProjectUnlink({ name: 'anything' });

    expect(exit).toBe(1);
    const err = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(err).toMatch(/not registered/);
  });

  it('exits 1 with helpful guidance when config.json does not exist', async () => {
    // No writeConfig — file is absent.
    const exit = await runProjectUnlink({ name: 'mystaging' });
    expect(exit).toBe(1);

    const errOutput = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(errOutput).toMatch(/config\.json|not registered|not linked/i);
  });

  it('exits 1 with a hard error (no overwrite) when config.json contains invalid JSON', async () => {
    await mkdir(join(testDir, '.notickets'), { recursive: true });
    await writeFile(join(testDir, '.notickets', 'config.json'), '{nope');

    const exit = await runProjectUnlink({ name: 'mystaging' });

    expect(exit).toBe(1);
    const errOutput = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(errOutput).toMatch(/invalid JSON/);

    // Untouched
    const onDisk = await readFile(join(testDir, '.notickets', 'config.json'), 'utf-8');
    expect(onDisk).toBe('{nope');
  });

  it('prints success confirmation on stdout', async () => {
    await writeConfig({
      profiles: {},
      projects: { mystaging: { profile: 'staging', pushToken: 'nt_push_x' } },
    });

    await runProjectUnlink({ name: 'mystaging' });

    const out = logSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(out).toMatch(/mystaging/);
    expect(out).toMatch(/unlinked|removed/i);
  });
});
