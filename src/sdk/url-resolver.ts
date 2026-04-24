import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import { DEFAULT_API_URL } from './auth.js';

export const DEFAULT_AUTH_URL = 'https://app.no-tickets.com/api/auth/cli';

export interface ResolvedUrls {
  readonly apiUrl: string;
  readonly authUrl: string;
  readonly source: 'profile' | 'env' | 'default';
}

interface ProfileConfig {
  readonly apiUrl: string;
  readonly authUrl: string;
}

interface ConfigFile {
  readonly profiles?: Readonly<Record<string, ProfileConfig>>;
}

function configDir(): string {
  const home = process.env['NO_TICKETS_HOME'] || os.homedir();
  return path.join(home, '.notickets');
}

function configPath(): string {
  return path.join(configDir(), 'config.json');
}

function isHttpUrl(value: unknown): value is string {
  if (typeof value !== 'string') return false;
  try {
    const u = new URL(value);
    return u.protocol === 'http:' || u.protocol === 'https:';
  } catch {
    return false;
  }
}

function isProfileConfig(value: unknown): value is ProfileConfig {
  if (typeof value !== 'object' || value === null) return false;
  const v = value as Record<string, unknown>;
  return isHttpUrl(v['apiUrl']) && isHttpUrl(v['authUrl']);
}

class MalformedConfigError extends Error {}

/** Read ~/.notickets/config.json. Distinguishes three states:
 *  - `{ exists: false }` — file missing; caller renders the "create with…" hint
 *  - throws MalformedConfigError — file present but unparseable
 *  - `{ exists: true, config }` — parsed (may still lack the requested profile) */
function readConfigFile(): { config: ConfigFile; exists: boolean } {
  const file = configPath();
  if (!fs.existsSync(file)) return { config: {}, exists: false };
  let raw: string;
  try {
    raw = fs.readFileSync(file, 'utf-8');
  } catch (err) {
    throw new MalformedConfigError(
      `${file} could not be read: ${err instanceof Error ? err.message : String(err)}`,
    );
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (err) {
    throw new MalformedConfigError(
      `${file} contains invalid JSON: ${err instanceof Error ? err.message : String(err)}`,
    );
  }
  if (typeof parsed !== 'object' || parsed === null) {
    return { config: {}, exists: true };
  }
  return { config: parsed as ConfigFile, exists: true };
}

/**
 * Load a named profile from ~/.notickets/config.json. Throws a user-facing
 * Error with a helpful message if the file is missing, malformed, or the
 * profile is absent / has invalid URLs.
 */
export function loadProfile(name: string): ProfileConfig {
  const { config, exists } = readConfigFile();
  if (!exists) {
    throw new Error(
      `profile "${name}" not found: ${configPath()} does not exist.\n` +
        `Create it with:\n` +
        `  { "profiles": { "${name}": { "apiUrl": "https://…", "authUrl": "https://…" } } }`,
    );
  }
  const profiles = config.profiles;
  const profile = profiles?.[name];
  if (profile === undefined) {
    const available = profiles ? Object.keys(profiles).join(', ') : '';
    const availableHint = available.length > 0 ? ` Available: ${available}.` : '';
    throw new Error(`profile "${name}" not found in ${configPath()}.${availableHint}`);
  }
  if (!isProfileConfig(profile)) {
    throw new Error(
      `profile "${name}" in ${configPath()} is invalid: apiUrl and authUrl must be http(s) URL strings.`,
    );
  }
  return profile;
}

/**
 * Resolve { apiUrl, authUrl } from (in priority order):
 *   1. --profile <name> (loads from ~/.notickets/config.json)
 *   2. NO_TICKETS_API_URL + NO_TICKETS_AUTH_URL env vars
 *   3. Production defaults
 *
 * Pair-validation: setting exactly one of the env vars throws — that's
 * the most common typo trap.
 */
export function resolveUrls(options: { readonly profile?: string }): ResolvedUrls {
  if (options.profile !== undefined) {
    const p = loadProfile(options.profile);
    return { apiUrl: p.apiUrl, authUrl: p.authUrl, source: 'profile' };
  }

  const envApi = (process.env['NO_TICKETS_API_URL'] ?? '').trim();
  const envAuth = (process.env['NO_TICKETS_AUTH_URL'] ?? '').trim();
  const apiSet = envApi.length > 0;
  const authSet = envAuth.length > 0;

  if (apiSet !== authSet) {
    const which = apiSet ? 'NO_TICKETS_API_URL' : 'NO_TICKETS_AUTH_URL';
    const value = apiSet ? envApi : envAuth;
    const missing = apiSet ? 'NO_TICKETS_AUTH_URL' : 'NO_TICKETS_API_URL';
    throw new Error(
      `${which}=${JSON.stringify(value)} is set but ${missing} is not. ` +
        `Set both (or neither) so the API and auth flow agree on which environment to use.`,
    );
  }

  if (apiSet && authSet) {
    return { apiUrl: envApi, authUrl: envAuth, source: 'env' };
  }

  return { apiUrl: DEFAULT_API_URL, authUrl: DEFAULT_AUTH_URL, source: 'default' };
}
