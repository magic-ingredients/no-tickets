import type { SessionState } from './core/types.js';

interface AgentEnvCheck {
  readonly env: string;
  readonly agent: string;
}

const AGENT_CHECKS: readonly AgentEnvCheck[] = [
  { env: 'CLAUDE_SESSION_ID', agent: 'claude-code' },
  { env: 'CURSOR_SESSION_ID', agent: 'cursor' },
  { env: 'WINDSURF_SESSION_ID', agent: 'windsurf' },
];

/**
 * Detect which AI tool is running from environment variables.
 * Returns session state with agent identity.
 */
export function detectAgent(): SessionState {
  for (const check of AGENT_CHECKS) {
    if (process.env[check.env]) {
      return {
        agent: check.agent,
        agentType: 'agent',
        active: true,
        since: new Date().toISOString(),
      };
    }
  }

  return {
    agent: 'unknown',
    agentType: 'human',
    active: true,
    since: new Date().toISOString(),
  };
}
