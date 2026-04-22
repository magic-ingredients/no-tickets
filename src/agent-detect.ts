import type { Session, PushEnvironment } from './core/types.js';

interface AgentEnvCheck {
  readonly env: string;
  readonly agent: string;
  readonly vendor: string;
}

const AGENT_CHECKS: readonly AgentEnvCheck[] = [
  { env: 'CLAUDE_SESSION_ID', agent: 'claude-code', vendor: 'anthropic' },
  { env: 'CURSOR_SESSION_ID', agent: 'cursor', vendor: 'cursor' },
  { env: 'WINDSURF_SESSION_ID', agent: 'windsurf', vendor: 'codeium' },
];

interface CiProviderCheck {
  readonly env: string;
  readonly provider: string;
}

const CI_PROVIDER_CHECKS: readonly CiProviderCheck[] = [
  { env: 'GITHUB_ACTIONS', provider: 'github-actions' },
  { env: 'GITLAB_CI', provider: 'gitlab' },
  { env: 'CIRCLECI', provider: 'circleci' },
  { env: 'JENKINS_URL', provider: 'jenkins' },
  { env: 'BUILDKITE', provider: 'buildkite' },
  { env: 'TRAVIS', provider: 'travis' },
];

function detectCiProvider(): string | undefined {
  for (const check of CI_PROVIDER_CHECKS) {
    if (process.env[check.env]) {
      return check.provider;
    }
  }
  return undefined;
}

function detectEnvironment(): PushEnvironment {
  const ci = Boolean(process.env['CI']);
  return {
    os: process.platform,
    runtime: process.version,
    ci,
    ciProvider: ci ? detectCiProvider() : undefined,
  };
}

/**
 * Detect which AI tool is running from environment variables.
 * Returns a v2 Session with auto-enriched environment and vendor.
 */
export function detectAgent(): Session {
  const environment = detectEnvironment();

  for (const check of AGENT_CHECKS) {
    if (process.env[check.env]) {
      return {
        agent: check.agent,
        agentType: 'agent',
        vendor: check.vendor,
        environment,
      };
    }
  }

  return {
    agent: 'unknown',
    agentType: 'human',
    environment,
  };
}
