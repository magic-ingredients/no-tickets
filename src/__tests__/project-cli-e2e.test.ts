import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runCli } from '../cli.js';

// End-to-end test for the cli.ts dispatcher → project handler routing.
// Verifies KNOWN_COMMANDS includes 'project', VALUE_FLAGS picks up
// '--token' as a value-bearing flag, handleProject picks the right
// subcommand handler, and the resulting config.json on disk matches
// what the user asked for.

let testDir: string;
let logSpy: ReturnType<typeof vi.spyOn>;
let errSpy: ReturnType<typeof vi.spyOn>;

beforeEach(async () => {
  testDir = await mkdtemp(join(tmpdir(), 'nt-project-cli-e2e-'));
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

describe('project command — runCli dispatcher routing', () => {
  it('runCli([project, link, name, --profile, staging, --token, nt_push_xxx]) writes the entry', async () => {
    await writeConfig({
      profiles: { staging: { apiUrl: 'https://x', authUrl: 'https://x/auth' } },
    });

    await runCli(['project', 'link', 'mystaging', '--profile', 'staging', '--token', 'nt_push_xxx']);

    expect(process.exitCode).toBeFalsy();
    const config = await readConfig();
    expect(config['projects']).toEqual({
      mystaging: { profile: 'staging', pushToken: 'nt_push_xxx' },
    });
  });

  it('runCli([project, list]) prints registered projects', async () => {
    await writeConfig({
      profiles: { staging: { apiUrl: 'https://x', authUrl: 'https://x/auth' } },
      projects: { myapp: { profile: 'staging', pushToken: 'nt_push_aaaa1234' } },
    });

    await runCli(['project', 'list']);

    expect(process.exitCode).toBeFalsy();
    const out = logSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(out).toMatch(/myapp/);
    expect(out).toMatch(/staging/);
  });

  it('runCli([project, unlink, name]) removes the entry and exits 0', async () => {
    await writeConfig({
      profiles: {},
      projects: { goodbye: { profile: 'staging', pushToken: 'nt_push_g' } },
    });

    await runCli(['project', 'unlink', 'goodbye']);

    expect(process.exitCode).toBeFalsy();
    const config = await readConfig();
    expect(config['projects']).toEqual({});
  });

  it('runCli([project, garbage]) exits 1 with a "Unknown project subcommand" error', async () => {
    await runCli(['project', 'garbage']);

    expect(process.exitCode).toBe(1);
    const err = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(err).toMatch(/Unknown project subcommand/);
    expect(err).toMatch(/link.*list.*unlink/);
  });

  it('runCli([project]) (no subcommand) exits 1 with the same usage error', async () => {
    await runCli(['project']);

    expect(process.exitCode).toBe(1);
    const err = errSpy.mock.calls.map((c: unknown[]) => String(c[0])).join('\n');
    expect(err).toMatch(/Unknown project subcommand/);
  });

  it('--token <value> parses correctly (was added to VALUE_FLAGS)', async () => {
    await writeConfig({
      profiles: { staging: { apiUrl: 'https://x', authUrl: 'https://x/auth' } },
    });

    // Without 'token' in VALUE_FLAGS, the parser would eat --token as a
    // boolean and treat 'nt_push_xxx' as a positional. The link handler
    // would then see token='' and fail. Pin the parsing.
    await runCli(['project', 'link', 'mystaging', '--profile', 'staging', '--token', 'nt_push_xxx']);

    expect(process.exitCode).toBeFalsy();
    const config = await readConfig();
    expect((config['projects'] as Record<string, { pushToken: string }>)['mystaging']?.pushToken).toBe(
      'nt_push_xxx',
    );
  });
});
