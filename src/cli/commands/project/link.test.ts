import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mkdtemp, mkdir, writeFile, readFile, rm, stat } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runProjectLink } from './link.js';

let testDir: string;
let logSpy: ReturnType<typeof vi.spyOn>;
let errSpy: ReturnType<typeof vi.spyOn>;

beforeEach(async () => {
  testDir = await mkdtemp(join(tmpdir(), 'nt-project-link-'));
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

const VALID_PROFILES = {
  profiles: {
    staging: {
      apiUrl: 'https://api-staging.example.com',
      authUrl: 'https://app-staging.example.com/api/auth/cli',
    },
  },
};

describe('runProjectLink', () => {
  it('writes a new project entry into ~/.notickets/config.json', async () => {
    await writeConfig(VALID_PROFILES);

    const exit = await runProjectLink({
      name: 'mystaging',
      profile: 'staging',
      token: 'nt_push_xxx',
    });

    expect(exit).toBe(0);
    const config = await readConfig();
    expect(config['projects']).toEqual({
      mystaging: { profile: 'staging', pushToken: 'nt_push_xxx' },
    });
  });

  it('preserves existing profiles when adding the first project entry', async () => {
    await writeConfig(VALID_PROFILES);

    await runProjectLink({ name: 'a', profile: 'staging', token: 'nt_push_a' });

    const config = await readConfig();
    expect(config['profiles']).toEqual(VALID_PROFILES.profiles);
  });

  it('preserves existing project entries when linking another', async () => {
    await writeConfig({
      ...VALID_PROFILES,
      projects: { existing: { profile: 'staging', pushToken: 'nt_push_existing' } },
    });

    await runProjectLink({ name: 'newone', profile: 'staging', token: 'nt_push_new' });

    const config = await readConfig();
    expect(config['projects']).toEqual({
      existing: { profile: 'staging', pushToken: 'nt_push_existing' },
      newone: { profile: 'staging', pushToken: 'nt_push_new' },
    });
  });

  it('writes config.json with file mode 0600 (secret-bearing)', async () => {
    await writeConfig(VALID_PROFILES);

    await runProjectLink({ name: 'a', profile: 'staging', token: 'nt_push_a' });

    const stats = await stat(join(testDir, '.notickets', 'config.json'));
    // Mask out the file-type bits; check the permission bits only.
    expect(stats.mode & 0o777).toBe(0o600);
  });

  it('exits 1 with a helpful error when the referenced profile is not defined', async () => {
    await writeConfig(VALID_PROFILES);

    const exit = await runProjectLink({
      name: 'mystaging',
      profile: 'no-such-profile',
      token: 'nt_push_xxx',
    });

    expect(exit).toBe(1);
    const errOutput = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(errOutput).toMatch(/no-such-profile/);
    expect(errOutput).toMatch(/profile/);

    // Did not modify config.json on the failure path
    const config = await readConfig();
    expect(config['projects']).toBeUndefined();
  });

  it('exits 1 by default when a project of the same name already exists (no --force)', async () => {
    await writeConfig({
      ...VALID_PROFILES,
      projects: { mystaging: { profile: 'staging', pushToken: 'nt_push_existing' } },
    });

    const exit = await runProjectLink({
      name: 'mystaging',
      profile: 'staging',
      token: 'nt_push_new',
    });

    expect(exit).toBe(1);
    const errOutput = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(errOutput).toMatch(/already linked/);
    expect(errOutput).toMatch(/--force/);

    // Token NOT overwritten on the rejection path
    const config = await readConfig();
    expect((config['projects'] as Record<string, { pushToken: string }>)['mystaging']?.pushToken).toBe(
      'nt_push_existing',
    );
  });

  it('overwrites an existing entry when --force is set', async () => {
    await writeConfig({
      ...VALID_PROFILES,
      projects: { mystaging: { profile: 'staging', pushToken: 'nt_push_old' } },
    });

    const exit = await runProjectLink({
      name: 'mystaging',
      profile: 'staging',
      token: 'nt_push_new',
      force: true,
    });

    expect(exit).toBe(0);
    const config = await readConfig();
    expect((config['projects'] as Record<string, { pushToken: string }>)['mystaging']?.pushToken).toBe(
      'nt_push_new',
    );
  });

  it('exits 1 when name is empty', async () => {
    await writeConfig(VALID_PROFILES);

    const exit = await runProjectLink({ name: '', profile: 'staging', token: 'nt_push_x' });
    expect(exit).toBe(1);
  });

  it('exits 1 when token is empty', async () => {
    await writeConfig(VALID_PROFILES);

    const exit = await runProjectLink({ name: 'a', profile: 'staging', token: '' });
    expect(exit).toBe(1);
  });

  it('exits 1 when profile is empty', async () => {
    await writeConfig(VALID_PROFILES);

    const exit = await runProjectLink({ name: 'a', profile: '', token: 'nt_push_x' });
    expect(exit).toBe(1);
  });

  it('exits 1 with guidance when ~/.notickets/config.json does not exist (no profiles to reference)', async () => {
    // No writeConfig — user hasn't created any profiles yet.
    const exit = await runProjectLink({
      name: 'mystaging',
      profile: 'staging',
      token: 'nt_push_x',
    });

    expect(exit).toBe(1);
    const errOutput = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(errOutput).toMatch(/config\.json/);
  });

  it('exits 1 with a hard error (no overwrite) when config.json contains invalid JSON', async () => {
    // Critical: the previous reader silently treated parse errors as "empty
    // config" and the next link would have rewritten the file from scratch
    // — wiping profiles + other projects. Hard-fail prevents that.
    await mkdir(join(testDir, '.notickets'), { recursive: true });
    await writeFile(join(testDir, '.notickets', 'config.json'), '{not valid: json}');

    const exit = await runProjectLink({
      name: 'mystaging',
      profile: 'staging',
      token: 'nt_push_x',
    });

    expect(exit).toBe(1);
    const errOutput = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(errOutput).toMatch(/invalid JSON/);
    expect(errOutput).toMatch(/Refusing to proceed/);

    // Critical: file untouched
    const onDisk = await readFile(join(testDir, '.notickets', 'config.json'), 'utf-8');
    expect(onDisk).toBe('{not valid: json}');
  });

  it('exits 1 with a hard error when config.json root is not an object', async () => {
    await mkdir(join(testDir, '.notickets'), { recursive: true });
    await writeFile(join(testDir, '.notickets', 'config.json'), '"oops"');

    const exit = await runProjectLink({
      name: 'mystaging',
      profile: 'staging',
      token: 'nt_push_x',
    });

    expect(exit).toBe(1);
    const errOutput = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(errOutput).toMatch(/not an object/);
  });

  it('treats profiles: <non-object> as "no profiles defined" rather than crashing on Object.keys', async () => {
    // Defensive cast — a malformed `profiles: "oops"` should produce the
    // user-friendly "profile not defined" path, not a TypeError.
    await writeConfig({ profiles: 'oops' });

    const exit = await runProjectLink({
      name: 'mystaging',
      profile: 'staging',
      token: 'nt_push_x',
    });

    expect(exit).toBe(1);
    const errOutput = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(errOutput).toMatch(/profile.*not defined/i);
  });

  it('prints success confirmation on stdout (with masked token, never the full secret)', async () => {
    await writeConfig(VALID_PROFILES);

    await runProjectLink({
      name: 'mystaging',
      profile: 'staging',
      token: 'nt_push_a0e79856da36a60367c38def8ccac62e85b79d81a46863338b21fe86f29ae0c9',
    });

    const out = logSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(out).toMatch(/mystaging/);
    expect(out).toMatch(/staging/);
    // The full secret must NOT appear in stdout
    expect(out).not.toContain('a0e79856da36a60367c38def8ccac62e85b79d81a46863338b21fe86f29ae0c9');
  });
});
