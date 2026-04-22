import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runCli } from '../cli.js';

const EPIC_CONTENT = `---
id: auth
type: epic
title: Authentication
status: in_progress
created: 2026-04-22
updated: 2026-04-22
---
# Authentication
`;

const FEATURE_CONTENT = `---
id: login
type: feature
epic: auth
title: Login Flow
phase: development
status: in_progress
created: 2026-04-22
updated: 2026-04-22
---
# Login Flow

## Tasks

### 1. Build login form
status: not_started
`;

let testDir: string;
let fetchSpy: ReturnType<typeof vi.fn>;
let logSpy: ReturnType<typeof vi.spyOn>;
let originalCwd: string;

function jsonResponse(body: unknown, status = 200) {
  return Promise.resolve(new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  }));
}

beforeEach(async () => {
  testDir = await mkdtemp(join(tmpdir(), 'nt-push-e2e-'));
  await mkdir(join(testDir, '.notickets', 'auth'), { recursive: true });
  await writeFile(join(testDir, '.notickets', 'auth', 'epic.md'), EPIC_CONTENT);
  await writeFile(join(testDir, '.notickets', 'auth', 'login.md'), FEATURE_CONTENT);

  fetchSpy = vi.fn().mockReturnValue(jsonResponse({ success: true, changesApplied: 2, eventsGenerated: 1 }));
  vi.stubGlobal('fetch', fetchSpy);

  logSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
  vi.spyOn(console, 'error').mockImplementation(() => {});

  vi.stubEnv('NO_TICKETS_TOKEN', 'nt_push_test123');
  vi.stubEnv('NO_TICKETS_PROJECT_ID', 'proj-e2e');
  vi.stubEnv('NO_TICKETS_API_URL', 'https://api.test.com');

  originalCwd = process.cwd();
  process.chdir(testDir);
  process.exitCode = undefined;
});

afterEach(async () => {
  process.chdir(originalCwd);
  vi.restoreAllMocks();
  vi.unstubAllEnvs();
  await rm(testDir, { recursive: true, force: true });
});

describe('push command e2e', () => {
  it('reads .notickets/ files and sends v2 push payload to API', async () => {
    await runCli(['push']);

    expect(fetchSpy).toHaveBeenCalledOnce();
    const [url, init] = fetchSpy.mock.calls[0] as [string, RequestInit];
    expect(url).toBe('https://api.test.com/api/v1/push');
    expect(init.method).toBe('POST');

    const body = JSON.parse(init.body as string);
    expect(body.projectId).toBe('proj-e2e');
    expect(body.session).toBeDefined();
    expect(body.session.agentType).toBeDefined();
    expect(body.work.entities.length).toBeGreaterThan(0);

    const types = body.work.entities.map((e: { type: string }) => e.type);
    expect(types).toContain('epic');
    expect(types).toContain('feature');
    expect(types).toContain('task');
  });

  it('sends Bearer auth header from NO_TICKETS_TOKEN', async () => {
    await runCli(['push']);

    const [, init] = fetchSpy.mock.calls[0] as [string, RequestInit];
    const headers = init.headers as Record<string, string>;
    expect(headers['Authorization']).toBe('Bearer nt_push_test123');
  });

  it('prints API response to stdout', async () => {
    await runCli(['push']);

    expect(logSpy).toHaveBeenCalledWith(
      JSON.stringify({ success: true, changesApplied: 2, eventsGenerated: 1 }),
    );
  });

  it('--dry-run prints payload without calling API', async () => {
    await runCli(['push', '--dry-run']);

    expect(fetchSpy).not.toHaveBeenCalled();
    expect(logSpy).toHaveBeenCalledOnce();

    const output = JSON.parse(logSpy.mock.calls[0]![0] as string);
    expect(output.projectId).toBe('proj-e2e');
    expect(output.work.entities.length).toBeGreaterThan(0);
  });

  it('fails with clear error when NO_TICKETS_PROJECT_ID is missing', async () => {
    vi.stubEnv('NO_TICKETS_PROJECT_ID', '');

    await expect(runCli(['push'])).rejects.toThrow('NO_TICKETS_PROJECT_ID');
  });

  it('handles API error responses', async () => {
    fetchSpy.mockReturnValue(jsonResponse({ error: 'Invalid payload' }, 400));

    await expect(runCli(['push'])).rejects.toThrow('400');
  });

  it('sends auto-enriched session with environment info', async () => {
    await runCli(['push']);

    const body = JSON.parse((fetchSpy.mock.calls[0] as [string, RequestInit])[1].body as string);
    expect(body.session.environment).toBeDefined();
    expect(body.session.environment.os).toBe(process.platform);
    expect(body.session.environment.runtime).toBe(process.version);
  });

  it('handles empty .notickets/ directory gracefully', async () => {
    await rm(join(testDir, '.notickets'), { recursive: true, force: true });
    await mkdir(join(testDir, '.notickets'), { recursive: true });

    await runCli(['push', '--dry-run']);

    const output = JSON.parse(logSpy.mock.calls[0]![0] as string);
    expect(output.projectId).toBe('proj-e2e');
    expect(output.work).toBeUndefined();
  });
});
