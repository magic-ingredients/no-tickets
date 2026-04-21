import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { detectAgent } from '../agent-detect.js';

const AGENT_ENV_VARS = ['CLAUDE_SESSION_ID', 'CURSOR_SESSION_ID', 'WINDSURF_SESSION_ID'];
const CI_ENV_VARS = ['CI', 'GITHUB_ACTIONS', 'GITLAB_CI', 'CIRCLECI', 'JENKINS_URL', 'BUILDKITE', 'TRAVIS'];

describe('detectAgent', () => {
  beforeEach(() => {
    for (const env of [...AGENT_ENV_VARS, ...CI_ENV_VARS]) {
      vi.stubEnv(env, '');
      delete process.env[env];
    }
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('detects claude-code from CLAUDE_SESSION_ID', () => {
    vi.stubEnv('CLAUDE_SESSION_ID', 'session-123');
    const session = detectAgent();
    expect(session.agent).toBe('claude-code');
    expect(session.agentType).toBe('agent');
  });

  it('detects cursor from CURSOR_SESSION_ID', () => {
    vi.stubEnv('CURSOR_SESSION_ID', 'cursor-456');
    const session = detectAgent();
    expect(session.agent).toBe('cursor');
    expect(session.agentType).toBe('agent');
  });

  it('detects windsurf from WINDSURF_SESSION_ID', () => {
    vi.stubEnv('WINDSURF_SESSION_ID', 'ws-789');
    const session = detectAgent();
    expect(session.agent).toBe('windsurf');
    expect(session.agentType).toBe('agent');
  });

  it('returns unknown agent when no env vars set', () => {
    const session = detectAgent();
    expect(session.agent).toBe('unknown');
    expect(session.agentType).toBe('human');
  });

  it('prefers claude-code over cursor when both set', () => {
    vi.stubEnv('CLAUDE_SESSION_ID', 'claude');
    vi.stubEnv('CURSOR_SESSION_ID', 'cursor');
    expect(detectAgent().agent).toBe('claude-code');
  });

  it('prefers cursor over windsurf when both set', () => {
    vi.stubEnv('CURSOR_SESSION_ID', 'cursor');
    vi.stubEnv('WINDSURF_SESSION_ID', 'windsurf');
    expect(detectAgent().agent).toBe('cursor');
  });

  describe('vendor derivation', () => {
    it('derives anthropic vendor from claude-code', () => {
      vi.stubEnv('CLAUDE_SESSION_ID', 'session-123');
      expect(detectAgent().vendor).toBe('anthropic');
    });

    it('derives cursor vendor from cursor', () => {
      vi.stubEnv('CURSOR_SESSION_ID', 'cursor-456');
      expect(detectAgent().vendor).toBe('cursor');
    });

    it('derives codeium vendor from windsurf', () => {
      vi.stubEnv('WINDSURF_SESSION_ID', 'ws-789');
      expect(detectAgent().vendor).toBe('codeium');
    });

    it('returns undefined vendor for unknown agent', () => {
      expect(detectAgent().vendor).toBeUndefined();
    });
  });

  describe('environment auto-enrichment', () => {
    it('populates os from process.platform', () => {
      const session = detectAgent();
      expect(session.environment?.os).toBe(process.platform);
    });

    it('populates runtime from process.version', () => {
      const session = detectAgent();
      expect(session.environment?.runtime).toBe(process.version);
    });

    it('sets ci to false when CI env var is not set', () => {
      const session = detectAgent();
      expect(session.environment?.ci).toBe(false);
    });

    it('sets ci to true when CI env var is set', () => {
      vi.stubEnv('CI', 'true');
      const session = detectAgent();
      expect(session.environment?.ci).toBe(true);
    });

    it('detects github-actions CI provider', () => {
      vi.stubEnv('CI', 'true');
      vi.stubEnv('GITHUB_ACTIONS', 'true');
      expect(detectAgent().environment?.ciProvider).toBe('github-actions');
    });

    it('detects gitlab CI provider', () => {
      vi.stubEnv('CI', 'true');
      vi.stubEnv('GITLAB_CI', 'true');
      expect(detectAgent().environment?.ciProvider).toBe('gitlab');
    });

    it('detects circleci CI provider', () => {
      vi.stubEnv('CI', 'true');
      vi.stubEnv('CIRCLECI', 'true');
      expect(detectAgent().environment?.ciProvider).toBe('circleci');
    });

    it('detects jenkins CI provider', () => {
      vi.stubEnv('CI', 'true');
      vi.stubEnv('JENKINS_URL', 'https://jenkins.example.com');
      expect(detectAgent().environment?.ciProvider).toBe('jenkins');
    });

    it('detects buildkite CI provider', () => {
      vi.stubEnv('CI', 'true');
      vi.stubEnv('BUILDKITE', 'true');
      expect(detectAgent().environment?.ciProvider).toBe('buildkite');
    });

    it('detects travis CI provider', () => {
      vi.stubEnv('CI', 'true');
      vi.stubEnv('TRAVIS', 'true');
      expect(detectAgent().environment?.ciProvider).toBe('travis');
    });

    it('returns undefined ciProvider when CI is set but no known provider', () => {
      vi.stubEnv('CI', 'true');
      expect(detectAgent().environment?.ciProvider).toBeUndefined();
    });

    it('returns undefined ciProvider when CI is not set', () => {
      expect(detectAgent().environment?.ciProvider).toBeUndefined();
    });
  });
});
