import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { detectSource } from '../agent-detect.js';
import { SDK_VERSION } from '../core/source.js';

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
  'BUILDKITE',
  'TRAVIS',
  'NO_TICKETS_INCLUDE_MACHINE',
];

describe('detectSource', () => {
  beforeEach(() => {
    for (const env of CI_ENV_VARS) {
      vi.stubEnv(env, '');
      delete process.env[env];
    }
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('defaults to name: sdk when no CI env vars are set', () => {
    const source = detectSource();
    expect(source.name).toBe('sdk');
  });

  it('populates sdkVersion from the SDK constant', () => {
    expect(detectSource().sdkVersion).toBe(SDK_VERSION);
  });

  it('returns name: ci when GITHUB_ACTIONS is set', () => {
    vi.stubEnv('CI', 'true');
    vi.stubEnv('GITHUB_ACTIONS', 'true');
    const source = detectSource();
    expect(source.name).toBe('ci');
    expect(source.attributes?.provider).toBe('github-actions');
  });

  it('returns name: ci with provider for GitLab', () => {
    vi.stubEnv('CI', 'true');
    vi.stubEnv('GITLAB_CI', 'true');
    expect(detectSource().attributes?.provider).toBe('gitlab');
  });

  it('returns name: ci with provider for CircleCI', () => {
    vi.stubEnv('CI', 'true');
    vi.stubEnv('CIRCLECI', 'true');
    expect(detectSource().attributes?.provider).toBe('circleci');
  });

  it('returns name: ci with provider for Jenkins', () => {
    vi.stubEnv('CI', 'true');
    vi.stubEnv('JENKINS_URL', 'https://jenkins.example.com');
    expect(detectSource().attributes?.provider).toBe('jenkins');
  });

  it('returns name: ci with provider for Buildkite', () => {
    vi.stubEnv('CI', 'true');
    vi.stubEnv('BUILDKITE', 'true');
    expect(detectSource().attributes?.provider).toBe('buildkite');
  });

  it('returns name: ci with provider for Travis', () => {
    vi.stubEnv('CI', 'true');
    vi.stubEnv('TRAVIS', 'true');
    expect(detectSource().attributes?.provider).toBe('travis');
  });

  it('populates runId and workflow for GitHub Actions', () => {
    vi.stubEnv('CI', 'true');
    vi.stubEnv('GITHUB_ACTIONS', 'true');
    vi.stubEnv('GITHUB_RUN_ID', '123456789');
    vi.stubEnv('GITHUB_WORKFLOW', 'CI Build');
    const source = detectSource();
    expect(source.attributes?.runId).toBe('123456789');
    expect(source.attributes?.workflow).toBe('CI Build');
  });

  it('populates runId for GitLab from CI_JOB_ID', () => {
    vi.stubEnv('CI', 'true');
    vi.stubEnv('GITLAB_CI', 'true');
    vi.stubEnv('CI_JOB_ID', 'gl-job-42');
    expect(detectSource().attributes?.runId).toBe('gl-job-42');
  });

  it('populates runId and workflow for CircleCI', () => {
    vi.stubEnv('CI', 'true');
    vi.stubEnv('CIRCLECI', 'true');
    vi.stubEnv('CIRCLE_BUILD_NUM', '999');
    vi.stubEnv('CIRCLE_JOB', 'test');
    const source = detectSource();
    expect(source.attributes?.runId).toBe('999');
    expect(source.attributes?.workflow).toBe('test');
  });

  it('omits runId/workflow when env vars are unset', () => {
    vi.stubEnv('CI', 'true');
    vi.stubEnv('GITHUB_ACTIONS', 'true');
    const source = detectSource();
    expect(source.attributes?.runId).toBeUndefined();
    expect(source.attributes?.workflow).toBeUndefined();
  });

  it('omits machine attribute when NO_TICKETS_INCLUDE_MACHINE is unset', () => {
    const source = detectSource();
    expect(source.attributes?.machine).toBeUndefined();
  });

  it('populates machine attribute (hashed) when NO_TICKETS_INCLUDE_MACHINE=1', () => {
    vi.stubEnv('NO_TICKETS_INCLUDE_MACHINE', '1');
    const source = detectSource();
    expect(source.attributes?.machine).toBeDefined();
    expect(typeof source.attributes?.machine).toBe('string');
    expect((source.attributes?.machine as string).length).toBeGreaterThan(0);
  });

  it('hashed machine differs from raw hostname', async () => {
    vi.stubEnv('NO_TICKETS_INCLUDE_MACHINE', '1');
    const os = await import('node:os');
    const source = detectSource();
    expect(source.attributes?.machine).not.toBe(os.hostname());
  });

  it('hashed machine is stable across calls (deterministic)', () => {
    vi.stubEnv('NO_TICKETS_INCLUDE_MACHINE', '1');
    const a = detectSource().attributes?.machine;
    const b = detectSource().attributes?.machine;
    expect(a).toBe(b);
  });

  it('passes the source through sourceSchema validation', async () => {
    const { sourceSchema } = await import('../core/source.js');
    expect(() => sourceSchema.parse(detectSource())).not.toThrow();
  });

  it('passes the source through sourceSchema validation when CI is set', async () => {
    vi.stubEnv('CI', 'true');
    vi.stubEnv('GITHUB_ACTIONS', 'true');
    vi.stubEnv('GITHUB_RUN_ID', '123');
    const { sourceSchema } = await import('../core/source.js');
    expect(() => sourceSchema.parse(detectSource())).not.toThrow();
  });
});
