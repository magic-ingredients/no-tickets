import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mkdtempSync, rmSync, mkdirSync, writeFileSync, existsSync, statSync } from 'node:fs';
import { hostname, tmpdir } from 'node:os';
import { join } from 'node:path';
import { detectSource } from '../agent-detect.js';
import { SDK_VERSION, sourceSchema } from '../core/source.js';

const CI_ENV_VARS = [
  'CI',
  'GITHUB_ACTIONS',
  'GITHUB_RUN_ID',
  'GITHUB_WORKFLOW',
  'GITLAB_CI',
  'CI_JOB_ID',
  'CI_PIPELINE_NAME',
  'CIRCLECI',
  'CIRCLE_BUILD_NUM',
  'CIRCLE_JOB',
  'JENKINS_URL',
  'BUILD_ID',
  'JOB_NAME',
  'BUILDKITE',
  'BUILDKITE_BUILD_ID',
  'BUILDKITE_PIPELINE_NAME',
  'TRAVIS',
  'TRAVIS_BUILD_ID',
  'TRAVIS_JOB_NAME',
  'NO_TICKETS_INCLUDE_MACHINE',
];

describe('detectSource', () => {
  let tempHome: string;

  beforeEach(() => {
    for (const env of CI_ENV_VARS) {
      delete process.env[env];
    }
    // Filesystem isolation: redirect HOME so machine-salt creation lands in a
    // temp dir rather than the developer's real ~/.notickets.
    tempHome = mkdtempSync(join(tmpdir(), 'no-tickets-test-'));
    vi.stubEnv('HOME', tempHome);
    vi.stubEnv('USERPROFILE', tempHome);
  });

  afterEach(() => {
    vi.unstubAllEnvs();
    rmSync(tempHome, { recursive: true, force: true });
  });

  it('defaults to name: sdk when no CI env vars are set', () => {
    expect(detectSource().name).toBe('sdk');
  });

  it('omits the attributes key entirely on a bare sdk source (no env vars)', () => {
    const source = detectSource();
    expect(source.attributes).toBeUndefined();
    expect(Object.keys(source)).not.toContain('attributes');
  });

  it('returns name: sdk when CI=true is set without a known provider', () => {
    vi.stubEnv('CI', 'true');
    const source = detectSource();
    expect(source.name).toBe('sdk');
    expect(source.attributes?.provider).toBeUndefined();
  });

  it('populates sdkVersion from the SDK constant', () => {
    expect(detectSource().sdkVersion).toBe(SDK_VERSION);
  });

  it('returns name: ci when GITHUB_ACTIONS is set', () => {
    vi.stubEnv('GITHUB_ACTIONS', 'true');
    const source = detectSource();
    expect(source.name).toBe('ci');
    expect(source.attributes?.provider).toBe('github-actions');
  });

  it('returns name: ci with provider for GitLab', () => {
    vi.stubEnv('GITLAB_CI', 'true');
    expect(detectSource().attributes?.provider).toBe('gitlab');
  });

  it('returns name: ci with provider for CircleCI', () => {
    vi.stubEnv('CIRCLECI', 'true');
    expect(detectSource().attributes?.provider).toBe('circleci');
  });

  it('returns name: ci with provider for Jenkins', () => {
    vi.stubEnv('JENKINS_URL', 'https://jenkins.example.com');
    expect(detectSource().attributes?.provider).toBe('jenkins');
  });

  it('returns name: ci with provider for Buildkite', () => {
    vi.stubEnv('BUILDKITE', 'true');
    expect(detectSource().attributes?.provider).toBe('buildkite');
  });

  it('returns name: ci with provider for Travis', () => {
    vi.stubEnv('TRAVIS', 'true');
    expect(detectSource().attributes?.provider).toBe('travis');
  });

  it('prefers github-actions over gitlab when both env vars are set', () => {
    vi.stubEnv('GITHUB_ACTIONS', 'true');
    vi.stubEnv('GITLAB_CI', 'true');
    expect(detectSource().attributes?.provider).toBe('github-actions');
  });

  it('populates runId and workflow for GitHub Actions', () => {
    vi.stubEnv('GITHUB_ACTIONS', 'true');
    vi.stubEnv('GITHUB_RUN_ID', '123456789');
    vi.stubEnv('GITHUB_WORKFLOW', 'CI Build');
    const source = detectSource();
    expect(source.attributes?.runId).toBe('123456789');
    expect(source.attributes?.workflow).toBe('CI Build');
  });

  it('populates runId and workflow for GitLab', () => {
    vi.stubEnv('GITLAB_CI', 'true');
    vi.stubEnv('CI_JOB_ID', 'gl-job-42');
    vi.stubEnv('CI_PIPELINE_NAME', 'main-pipeline');
    const source = detectSource();
    expect(source.attributes?.runId).toBe('gl-job-42');
    expect(source.attributes?.workflow).toBe('main-pipeline');
  });

  it('populates runId and workflow for CircleCI', () => {
    vi.stubEnv('CIRCLECI', 'true');
    vi.stubEnv('CIRCLE_BUILD_NUM', '999');
    vi.stubEnv('CIRCLE_JOB', 'test');
    const source = detectSource();
    expect(source.attributes?.runId).toBe('999');
    expect(source.attributes?.workflow).toBe('test');
  });

  it('populates runId and workflow for Jenkins', () => {
    vi.stubEnv('JENKINS_URL', 'https://jenkins.example.com');
    vi.stubEnv('BUILD_ID', 'jenkins-100');
    vi.stubEnv('JOB_NAME', 'main-pipeline');
    const source = detectSource();
    expect(source.attributes?.runId).toBe('jenkins-100');
    expect(source.attributes?.workflow).toBe('main-pipeline');
  });

  it('populates runId and workflow for Buildkite', () => {
    vi.stubEnv('BUILDKITE', 'true');
    vi.stubEnv('BUILDKITE_BUILD_ID', 'bk-uuid-1');
    vi.stubEnv('BUILDKITE_PIPELINE_NAME', 'release');
    const source = detectSource();
    expect(source.attributes?.runId).toBe('bk-uuid-1');
    expect(source.attributes?.workflow).toBe('release');
  });

  it('populates runId and workflow for Travis', () => {
    vi.stubEnv('TRAVIS', 'true');
    vi.stubEnv('TRAVIS_BUILD_ID', 'travis-1');
    vi.stubEnv('TRAVIS_JOB_NAME', 'unit');
    const source = detectSource();
    expect(source.attributes?.runId).toBe('travis-1');
    expect(source.attributes?.workflow).toBe('unit');
  });

  it('omits runId/workflow keys when env vars are unset', () => {
    vi.stubEnv('GITHUB_ACTIONS', 'true');
    const source = detectSource();
    expect(source.attributes).toEqual({ provider: 'github-actions' });
    expect(Object.keys(source.attributes ?? {})).not.toContain('runId');
    expect(Object.keys(source.attributes ?? {})).not.toContain('workflow');
  });

  it('omits machine attribute when NO_TICKETS_INCLUDE_MACHINE is unset', () => {
    expect(detectSource().attributes?.machine).toBeUndefined();
  });

  it('populates machine attribute (16-hex-char hash) when NO_TICKETS_INCLUDE_MACHINE=1', () => {
    vi.stubEnv('NO_TICKETS_INCLUDE_MACHINE', '1');
    expect(detectSource().attributes?.machine).toMatch(/^[0-9a-f]{16}$/);
  });

  it('hashed machine differs from raw hostname', () => {
    vi.stubEnv('NO_TICKETS_INCLUDE_MACHINE', '1');
    expect(detectSource().attributes?.machine).not.toBe(hostname());
  });

  it('hashed machine is stable across calls (deterministic with same salt + hostname)', () => {
    vi.stubEnv('NO_TICKETS_INCLUDE_MACHINE', '1');
    const a = detectSource().attributes?.machine;
    const b = detectSource().attributes?.machine;
    expect(a).toBe(b);
    expect(a).toMatch(/^[0-9a-f]{16}$/);
  });

  it('hash differs across persisted-salt values (proves salt is mixed in)', () => {
    vi.stubEnv('NO_TICKETS_INCLUDE_MACHINE', '1');
    // First run creates salt A, computes hash A.
    const hashA = detectSource().attributes?.machine as string;
    // Replace the salt file with a different value; same hostname → different hash.
    const saltPath = join(tempHome, '.notickets', '.machine-salt');
    writeFileSync(saltPath, 'totally-different-salt');
    const hashB = detectSource().attributes?.machine as string;
    expect(hashA).not.toBe(hashB);
  });

  it('persists the salt file under HOME/.notickets/.machine-salt', () => {
    vi.stubEnv('NO_TICKETS_INCLUDE_MACHINE', '1');
    detectSource();
    expect(existsSync(join(tempHome, '.notickets', '.machine-salt'))).toBe(true);
  });

  it('reuses an existing salt file rather than overwriting', () => {
    vi.stubEnv('NO_TICKETS_INCLUDE_MACHINE', '1');
    const dir = join(tempHome, '.notickets');
    mkdirSync(dir, { recursive: true });
    const saltPath = join(dir, '.machine-salt');
    writeFileSync(saltPath, 'preexisting-salt-value', { mode: 0o600 });
    const a = detectSource().attributes?.machine;
    const b = detectSource().attributes?.machine;
    expect(a).toBe(b);
  });

  it('regenerates salt when existing salt file is empty / whitespace-only', async () => {
    vi.stubEnv('NO_TICKETS_INCLUDE_MACHINE', '1');
    const dir = join(tempHome, '.notickets');
    mkdirSync(dir, { recursive: true });
    const saltPath = join(dir, '.machine-salt');
    writeFileSync(saltPath, '   \n\t  ', { mode: 0o600 });
    expect(detectSource().attributes?.machine).toMatch(/^[0-9a-f]{16}$/);
    const { readFileSync } = await import('node:fs');
    const newSalt = readFileSync(saltPath, 'utf-8').trim();
    expect(newSalt.length).toBeGreaterThan(0);
  });

  it('writes the salt file with restrictive permissions (0o600 on POSIX)', () => {
    vi.stubEnv('NO_TICKETS_INCLUDE_MACHINE', '1');
    detectSource();
    const saltPath = join(tempHome, '.notickets', '.machine-salt');
    if (process.platform !== 'win32') {
      // mode bits — only the lower 9 bits are interesting (rwxrwxrwx)
      const mode = statSync(saltPath).mode & 0o777;
      expect(mode).toBe(0o600);
    } else {
      expect(existsSync(saltPath)).toBe(true);
    }
  });

  it('falls back to USERPROFILE when HOME is unset (Windows-style env)', () => {
    vi.stubEnv('NO_TICKETS_INCLUDE_MACHINE', '1');
    delete process.env['HOME'];
    vi.stubEnv('USERPROFILE', tempHome);
    expect(detectSource().attributes?.machine).toMatch(/^[0-9a-f]{16}$/);
    expect(existsSync(join(tempHome, '.notickets', '.machine-salt'))).toBe(true);
  });

  it('does not throw and omits machine when home dir is unwritable', () => {
    vi.stubEnv('NO_TICKETS_INCLUDE_MACHINE', '1');
    vi.stubEnv('HOME', '/proc/nonexistent-readonly-path');
    vi.stubEnv('USERPROFILE', '/proc/nonexistent-readonly-path');
    expect(() => detectSource()).not.toThrow();
    expect(detectSource().attributes?.machine).toBeUndefined();
  });

  describe('schema conformance', () => {
    it('returns a Source that parses cleanly with no env vars', () => {
      const source = detectSource();
      const parsed = sourceSchema.parse(source);
      expect(parsed.name).toBe('sdk');
      expect(parsed.sdkVersion).toBe(SDK_VERSION);
    });

    it('returns a Source that parses cleanly with full CI attributes', () => {
      vi.stubEnv('GITHUB_ACTIONS', 'true');
      vi.stubEnv('GITHUB_RUN_ID', '123');
      vi.stubEnv('GITHUB_WORKFLOW', 'main');
      const parsed = sourceSchema.parse(detectSource());
      expect(parsed.name).toBe('ci');
      expect(parsed.attributes).toEqual({
        provider: 'github-actions',
        runId: '123',
        workflow: 'main',
      });
    });
  });
});
