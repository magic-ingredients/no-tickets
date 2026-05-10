import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mkdtempSync, rmSync, mkdirSync, writeFileSync, existsSync, statSync } from 'node:fs';
import { hostname, tmpdir } from 'node:os';
import { join } from 'node:path';
import { detectSource } from '../agent-detect.js';
import { SDK_VERSION, sourceSchema } from '../core/source.js';

// CI provenance is now caller-driven, not auto-detected. detectSource() must
// ignore these env vars entirely — surface-specific defaults ('cli', 'mcp',
// 'sdk') own the `source.name` decision, and CI scripts that want CI
// provenance supply it explicitly via PublishEvent.source.attributes or
// the CLI's --source-attribute flag.
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
    tempHome = mkdtempSync(join(tmpdir(), 'no-tickets-test-'));
    vi.stubEnv('HOME', tempHome);
    vi.stubEnv('USERPROFILE', tempHome);
  });

  afterEach(() => {
    vi.unstubAllEnvs();
    rmSync(tempHome, { recursive: true, force: true });
  });

  describe('default shape', () => {
    it('returns name: "sdk" when no env vars are set', () => {
      expect(detectSource().name).toBe('sdk');
    });

    it('omits the attributes key entirely on a bare default source', () => {
      const source = detectSource();
      expect(source.attributes).toBeUndefined();
      expect(Object.keys(source)).not.toContain('attributes');
    });

    it('populates sdkVersion from the SDK constant', () => {
      expect(detectSource().sdkVersion).toBe(SDK_VERSION);
    });
  });

  describe('CI env vars are no longer auto-detected', () => {
    // Caller-driven provenance only — self-hosted runners that miss the
    // canonical env var no longer get mislabeled, and devcontainers / `act`
    // / dev shells with a sticky GITHUB_ACTIONS=true don't silently flip
    // local work into CI provenance. The right knob is explicit:
    // `--source-attribute provider=github-actions` (CLI) or
    // `source.attributes.provider` (event envelope).
    const PROVIDERS: ReadonlyArray<readonly [string, string]> = [
      ['GITHUB_ACTIONS', 'true'],
      ['GITLAB_CI', 'true'],
      ['CIRCLECI', 'true'],
      ['JENKINS_URL', 'https://jenkins.example.com'],
      ['BUILDKITE', 'true'],
      ['TRAVIS', 'true'],
    ];

    it.each(PROVIDERS)('returns name: "sdk" even with %s set', (envVar, value) => {
      vi.stubEnv(envVar, value);
      expect(detectSource().name).toBe('sdk');
    });

    it('does NOT populate attributes.provider under any known CI env var', () => {
      // Stub them all at once — if ANY single one slipped a `provider`
      // attribute through, the assertion fails. The individual it.each
      // above pins the name-only behavior per provider for legibility.
      for (const [envVar, value] of PROVIDERS) {
        vi.stubEnv(envVar, value);
      }
      const source = detectSource();
      expect(source.attributes?.provider).toBeUndefined();
    });

    it('does NOT populate attributes.runId or workflow even when provider-specific run env vars are set', () => {
      vi.stubEnv('GITHUB_ACTIONS', 'true');
      vi.stubEnv('GITHUB_RUN_ID', '123');
      vi.stubEnv('GITHUB_WORKFLOW', 'build');
      const source = detectSource();
      expect(source.attributes?.runId).toBeUndefined();
      expect(source.attributes?.workflow).toBeUndefined();
    });
  });

  describe('machine-hash opt-in (preserved)', () => {
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
      const hashA = detectSource().attributes?.machine as string;
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
  });

  describe('schema conformance', () => {
    it('returns a Source that parses cleanly with no env vars', () => {
      const source = detectSource();
      const parsed = sourceSchema.parse(source);
      expect(parsed.name).toBe('sdk');
      expect(parsed.sdkVersion).toBe(SDK_VERSION);
    });

    it('returns a Source that parses cleanly with NO_TICKETS_INCLUDE_MACHINE=1', () => {
      vi.stubEnv('NO_TICKETS_INCLUDE_MACHINE', '1');
      const parsed = sourceSchema.parse(detectSource());
      expect(parsed.name).toBe('sdk');
      expect(parsed.attributes?.machine).toMatch(/^[0-9a-f]{16}$/);
    });
  });
});
