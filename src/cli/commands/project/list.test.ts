import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runProjectList } from './list.js';

let testDir: string;
let logSpy: ReturnType<typeof vi.spyOn>;

beforeEach(async () => {
  testDir = await mkdtemp(join(tmpdir(), 'nt-project-list-'));
  vi.stubEnv('NO_TICKETS_HOME', testDir);
  process.exitCode = undefined;
  logSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
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

describe('runProjectList', () => {
  it('prints registered projects with their profile name and a masked token', async () => {
    await writeConfig({
      profiles: {
        staging: { apiUrl: 'https://api', authUrl: 'https://app/auth' },
      },
      projects: {
        myapp: {
          profile: 'staging',
          pushToken: 'nt_push_a0e79856da36a60367c38def8ccac62e85b79d81a46863338b21fe86f29ae0c9',
        },
      },
    });

    const exit = await runProjectList();
    expect(exit).toBe(0);

    const out = logSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(out).toMatch(/myapp/);
    expect(out).toMatch(/staging/);
    // The full secret must NEVER appear; only the prefix + a tail snippet
    expect(out).not.toContain('a0e79856da36a60367c38def8ccac62e85b79d81a46863338b21fe86f29ae0c9');
    // Mask convention: nt_push_…<last 4>
    expect(out).toMatch(/nt_push_/);
  });

  it('shows zero projects with a "no projects registered" message when projects section is empty', async () => {
    await writeConfig({
      profiles: { staging: { apiUrl: 'https://x', authUrl: 'https://x/auth' } },
    });

    const exit = await runProjectList();
    expect(exit).toBe(0);

    const out = logSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(out).toMatch(/no projects/i);
  });

  it('exits 0 (not 1) and prints "no projects" when config.json does not exist', async () => {
    // No writeConfig — file is absent. List is informational, not an error path.
    const exit = await runProjectList();
    expect(exit).toBe(0);

    const out = logSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(out).toMatch(/no projects/i);
  });

  it('lists multiple projects in stable (alphabetic) order', async () => {
    await writeConfig({
      profiles: {
        staging: { apiUrl: 'https://x', authUrl: 'https://x/auth' },
        production: { apiUrl: 'https://y', authUrl: 'https://y/auth' },
      },
      projects: {
        zebra: { profile: 'staging', pushToken: 'nt_push_zzz' },
        alpha: { profile: 'production', pushToken: 'nt_push_aaa' },
        middle: { profile: 'staging', pushToken: 'nt_push_mmm' },
      },
    });

    await runProjectList();

    const out = logSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    const alphaIdx = out.indexOf('alpha');
    const middleIdx = out.indexOf('middle');
    const zebraIdx = out.indexOf('zebra');
    expect(alphaIdx).toBeLessThan(middleIdx);
    expect(middleIdx).toBeLessThan(zebraIdx);
  });

  it('handles a malformed entry (no pushToken) by listing what it can without crashing', async () => {
    await writeConfig({
      profiles: { staging: { apiUrl: 'https://x', authUrl: 'https://x/auth' } },
      projects: {
        broken: { profile: 'staging' /* no pushToken */ },
      },
    });

    const exit = await runProjectList();
    expect(exit).toBe(0);

    const out = logSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(out).toMatch(/broken/);
  });
});
