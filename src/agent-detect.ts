import { createHash, randomBytes } from 'node:crypto';
import { hostname, homedir } from 'node:os';
import { mkdirSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';

// Read HOME/USERPROFILE env vars first (testable via env-stubbing) before
// falling back to os.homedir(). ESM bindings on os.homedir cannot be spied.
function resolveHome(): string {
  return process.env['HOME'] ?? process.env['USERPROFILE'] ?? homedir();
}
import type { Session, PushEnvironment } from './core/types.js';
import { type Source, SDK_VERSION } from './core/source.js';

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

interface CiAttributeBindings {
  readonly provider: string;
  readonly runId?: string;
  readonly workflow?: string;
}

// Order matters: the first entry whose env var is set wins. GitHub Actions
// before GitLab so a runner that mistakenly has both vars set is identified
// as the more specific provider.
const CI_ATTRIBUTE_BINDINGS: ReadonlyArray<readonly [string, () => CiAttributeBindings]> = [
  [
    'GITHUB_ACTIONS',
    () => ({
      provider: 'github-actions',
      runId: process.env['GITHUB_RUN_ID'],
      workflow: process.env['GITHUB_WORKFLOW'],
    }),
  ],
  [
    'GITLAB_CI',
    () => ({
      provider: 'gitlab',
      runId: process.env['CI_JOB_ID'],
      workflow: process.env['CI_PIPELINE_NAME'],
    }),
  ],
  [
    'CIRCLECI',
    () => ({
      provider: 'circleci',
      runId: process.env['CIRCLE_BUILD_NUM'],
      workflow: process.env['CIRCLE_JOB'],
    }),
  ],
  [
    'JENKINS_URL',
    () => ({
      provider: 'jenkins',
      runId: process.env['BUILD_ID'],
      workflow: process.env['JOB_NAME'],
    }),
  ],
  [
    'BUILDKITE',
    () => ({
      provider: 'buildkite',
      runId: process.env['BUILDKITE_BUILD_ID'],
      workflow: process.env['BUILDKITE_PIPELINE_NAME'],
    }),
  ],
  [
    'TRAVIS',
    () => ({
      provider: 'travis',
      runId: process.env['TRAVIS_BUILD_ID'],
      workflow: process.env['TRAVIS_JOB_NAME'],
    }),
  ],
];

function detectCiAttributes(): CiAttributeBindings | null {
  for (const [envVar, binding] of CI_ATTRIBUTE_BINDINGS) {
    if (process.env[envVar]) return binding();
  }
  return null;
}

// Salt is generated once per installation and stored at ~/.notickets/.machine-salt.
// Hostname + salt → SHA-256 hex (truncated). Hostname alone is PII; salted hash
// is opaque to anyone without the local salt file.
function readOrCreateMachineSalt(): string {
  const dir = join(resolveHome(), '.notickets');
  const path = join(dir, '.machine-salt');
  if (existsSync(path)) {
    const existing = readFileSync(path, 'utf-8').trim();
    if (existing.length > 0) return existing;
  }
  mkdirSync(dir, { recursive: true });
  const salt = randomBytes(16).toString('hex');
  // Atomic create-or-fail: handles the race where two concurrent first-runs
  // both reach the write step. Loser re-reads the winner's salt.
  try {
    writeFileSync(path, salt, { mode: 0o600, flag: 'wx' });
    return salt;
  } catch {
    return readFileSync(path, 'utf-8').trim();
  }
}

function hashedMachine(): string {
  const salt = readOrCreateMachineSalt();
  return createHash('sha256').update(`${salt}:${hostname()}`).digest('hex').slice(0, 16);
}

/**
 * Detect a fully-formed Source for direct SDK use. Used by the publish-client
 * (Feature 2) to auto-fill source on every event when the caller doesn't
 * provide one.
 *
 * - `name: 'ci'` when a known CI provider env var is set; `attributes.provider`
 *   identifies which one, plus `runId`/`workflow` when the provider exposes them.
 * - `name: 'sdk'` otherwise (direct programmatic SDK use).
 * - `attributes.machine` populated only when `NO_TICKETS_INCLUDE_MACHINE=1`.
 *   Value is a hashed hostname using a per-installation salt (never raw hostname).
 */
export function detectSource(): Source {
  const attributes: Record<string, string | number | boolean> = {};

  const ci = detectCiAttributes();
  if (ci) {
    attributes['provider'] = ci.provider;
    if (ci.runId) attributes['runId'] = ci.runId;
    if (ci.workflow) attributes['workflow'] = ci.workflow;
  }

  if (process.env['NO_TICKETS_INCLUDE_MACHINE'] === '1') {
    // Best-effort: filesystem failures (read-only $HOME, missing perms, ...)
    // must not break the calling SDK's auto-fill. Drop the attribute silently.
    try {
      attributes['machine'] = hashedMachine();
    } catch {
      // intentional no-op
    }
  }

  const source: Source = {
    name: ci ? 'ci' : 'sdk',
    sdkVersion: SDK_VERSION,
  };

  if (Object.keys(attributes).length > 0) {
    return { ...source, attributes };
  }
  return source;
}

/**
 * Detect which AI tool is running from environment variables.
 * Returns a v2 Session with auto-enriched environment and vendor.
 *
 * @deprecated Use {@link detectSource} for the new envelope flow. Removed in
 * Feature 2 along with the push command.
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
