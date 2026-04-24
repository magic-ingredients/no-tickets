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

function isProfileConfig(value: unknown): value is ProfileConfig {
  if (typeof value !== 'object' || value === null) return false;
  const v = value as Record<string, unknown>;
  return typeof v['apiUrl'] === 'string' && typeof v['authUrl'] === 'string';
}

function readConfigFile(): { config: ConfigFile | null; exists: boolean } {
  const file = configPath();
  if (!fs.existsSync(file)) return { config: null, exists: false };
  try {
    const raw = fs.readFileSync(file, 'utf-8');
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== 'object' || parsed === null) {
      return { config: {}, exists: true };
    }
    return { config: parsed as ConfigFile, exists: true };
  } catch {
    return { config: {}, exists: true };
  }
}

/**
 * Load a named profile from ~/.notickets/config.json. Throws a user-facing
 * Error with a helpful message if the file or profile is missing.
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
  const profiles = config?.profiles;
  const profile = profiles?.[name];
  if (!isProfileConfig(profile)) {
    const available = profiles ? Object.keys(profiles).join(', ') : '';
    const availableHint = available.length > 0 ? ` Available: ${available}.` : '';
    throw new Error(`profile "${name}" not found in ${configPath()}.${availableHint}`);
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

  const envApi = process.env['NO_TICKETS_API_URL'];
  const envAuth = process.env['NO_TICKETS_AUTH_URL'];
  const apiSet = envApi !== undefined && envApi.length > 0;
  const authSet = envAuth !== undefined && envAuth.length > 0;

  if (apiSet !== authSet) {
    const which = apiSet ? 'NO_TICKETS_API_URL' : 'NO_TICKETS_AUTH_URL';
    const missing = apiSet ? 'NO_TICKETS_AUTH_URL' : 'NO_TICKETS_API_URL';
    throw new Error(
      `${which} is set but ${missing} is not. ` +
        `Set both (or neither) so the API and auth flow agree on which environment to use.`,
    );
  }

  if (apiSet && authSet) {
    return { apiUrl: envApi, authUrl: envAuth, source: 'env' };
  }

  return { apiUrl: DEFAULT_API_URL, authUrl: DEFAULT_AUTH_URL, source: 'default' };
}
