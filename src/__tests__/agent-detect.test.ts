import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { detectAgent } from '../agent-detect.js';

const AGENT_ENV_VARS = ['CLAUDE_SESSION_ID', 'CURSOR_SESSION_ID', 'WINDSURF_SESSION_ID'];

describe('detectAgent', () => {
  beforeEach(() => {
    // Clear all agent env vars to ensure test isolation
    for (const env of AGENT_ENV_VARS) {
      vi.stubEnv(env, '');
      delete process.env[env];
    }
  });

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('detects claude-code from CLAUDE_SESSION_ID', () => {
    vi.stubEnv('CLAUDE_SESSION_ID', 'session-123');
    const agent = detectAgent();
    expect(agent.agent).toBe('claude-code');
    expect(agent.agentType).toBe('agent');
  });

  it('detects cursor from CURSOR_SESSION_ID', () => {
    vi.stubEnv('CURSOR_SESSION_ID', 'cursor-456');
    const agent = detectAgent();
    expect(agent.agent).toBe('cursor');
    expect(agent.agentType).toBe('agent');
  });

  it('detects windsurf from WINDSURF_SESSION_ID', () => {
    vi.stubEnv('WINDSURF_SESSION_ID', 'ws-789');
    const agent = detectAgent();
    expect(agent.agent).toBe('windsurf');
    expect(agent.agentType).toBe('agent');
  });

  it('returns unknown agent when no env vars set', () => {
    const agent = detectAgent();
    expect(agent.agent).toBe('unknown');
    expect(agent.agentType).toBe('human');
  });

  it('returns active session with valid ISO date', () => {
    const agent = detectAgent();
    expect(agent.active).toBe(true);
    expect(agent.since).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/);
    expect(new Date(agent.since).getTime()).not.toBeNaN();
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
});
