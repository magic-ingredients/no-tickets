import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { runCli } from '../cli.js';

const VALID_EPIC = `---
id: auth
type: epic
title: Authentication
status: in_progress
created: 2026-04-22
updated: 2026-04-22
---
# Authentication
`;

const VALID_FEATURE = `---
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

### 1. Build form
status: not_started
`;

const INVALID_FEATURE = `---
id: INVALID ID
type: feature
epic: auth
title: Bad Feature
phase: development
status: in_progress
created: 2026-04-22
updated: 2026-04-22
---
`;

let testDir: string;
let logSpy: ReturnType<typeof vi.spyOn>;
let errorSpy: ReturnType<typeof vi.spyOn>;
let originalCwd: string;

beforeEach(async () => {
  testDir = await mkdtemp(join(tmpdir(), 'nt-validate-e2e-'));
  logSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
  errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  originalCwd = process.cwd();
  process.chdir(testDir);
  process.exitCode = undefined;
});

afterEach(async () => {
  process.chdir(originalCwd);
  vi.restoreAllMocks();
  await rm(testDir, { recursive: true, force: true });
});

describe('validate command e2e', () => {
  it('passes validation for well-formed .notickets/ files', async () => {
    await mkdir(join(testDir, '.notickets', 'auth'), { recursive: true });
    await writeFile(join(testDir, '.notickets', 'auth', 'epic.md'), VALID_EPIC);
    await writeFile(join(testDir, '.notickets', 'auth', 'login.md'), VALID_FEATURE);

    await runCli(['validate']);

    expect(logSpy).toHaveBeenCalledWith('Validation passed — no errors found.');
    expect(process.exitCode).toBeUndefined();
  });

  it('fails validation and prints errors for invalid files', async () => {
    await mkdir(join(testDir, '.notickets', 'auth'), { recursive: true });
    await writeFile(join(testDir, '.notickets', 'auth', 'epic.md'), VALID_EPIC);
    await writeFile(join(testDir, '.notickets', 'auth', 'bad.md'), INVALID_FEATURE);

    await runCli(['validate']);

    expect(process.exitCode).toBe(1);
    expect(errorSpy).toHaveBeenCalled();
    const errorOutput = errorSpy.mock.calls.map((c) => c[0]).join('\n');
    expect(errorOutput).toContain('ERROR');
  });

  it('handles missing .notickets/ directory gracefully', async () => {
    await runCli(['validate']);

    expect(logSpy).toHaveBeenCalledWith('Validation passed — no errors found.');
    expect(process.exitCode).toBeUndefined();
  });

  it('detects orphan features referencing non-existent epics', async () => {
    await mkdir(join(testDir, '.notickets', 'auth'), { recursive: true });
    const orphanFeature = VALID_FEATURE.replace('epic: auth', 'epic: ghost');
    await writeFile(join(testDir, '.notickets', 'auth', 'orphan.md'), orphanFeature);

    await runCli(['validate']);

    expect(process.exitCode).toBe(1);
    const errorOutput = errorSpy.mock.calls.map((c) => c[0]).join('\n');
    expect(errorOutput).toContain('ghost');
  });
});
