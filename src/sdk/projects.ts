import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import type { ClientOptions } from '../transport/client.js';
import { Client } from '../transport/client.js';

// Phase-1 project registry. Reads ~/.notickets/config.json and joins
// projects[name] with the profile referenced by projects[name].profile to
// produce the (token, apiUrl, authUrl) tuple a client needs at publish
// time. clientForProject(name) is the one-line factory production callers
// use:  await publish(clientForProject('myapp'), [...]).
//
// CI does not use this path — see publish-shared-surfaces.md "Target shape"
// for the env-var / --token-env-var paths reserved for CI.

export interface ResolvedProjectAuth {
  readonly token: string;
  readonly apiUrl: string;
  readonly authUrl: string;
}

interface ProjectEntry {
  readonly profile: string;
  readonly pushToken: string;
}

interface ProfileEntry {
  readonly apiUrl: string;
  readonly authUrl: string;
}

// Read with `unknown` fields and narrow at use-site — `parsed as ConfigFile`
// would let a malformed `projects: 42` survive to `Object.keys` / `Object.hasOwn`
// and then NPE on indexing. The runtime guards downstream do the narrowing.
interface ConfigFile {
  readonly profiles?: unknown;
  readonly projects?: unknown;
}

export class ProjectNotRegisteredError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'ProjectNotRegisteredError';
  }
}

function configDir(): string {
  const home = process.env['NO_TICKETS_HOME'] || os.homedir();
  return path.join(home, '.notickets');
}

function configPath(): string {
  return path.join(configDir(), 'config.json');
}

function readConfig(): { config: ConfigFile; exists: boolean } {
  const file = configPath();
  if (!fs.existsSync(file)) return { config: {}, exists: false };
  const raw = fs.readFileSync(file, 'utf-8');
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (err) {
    throw new Error(
      `${file} contains invalid JSON: ${err instanceof Error ? err.message : String(err)}`,
    );
  }
  if (typeof parsed !== 'object' || parsed === null) return { config: {}, exists: true };
  return { config: parsed as ConfigFile, exists: true };
}

function isProjectEntry(v: unknown): v is ProjectEntry {
  if (typeof v !== 'object' || v === null) return false;
  const e = v as Record<string, unknown>;
  return typeof e['profile'] === 'string' && typeof e['pushToken'] === 'string';
}

function isProfileEntry(v: unknown): v is ProfileEntry {
  if (typeof v !== 'object' || v === null) return false;
  const e = v as Record<string, unknown>;
  return typeof e['apiUrl'] === 'string' && typeof e['authUrl'] === 'string';
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

export function resolveProjectAuth(name: string): ResolvedProjectAuth {
  const { config, exists } = readConfig();
  if (!exists) {
    throw new ProjectNotRegisteredError(
      `project "${name}" not registered: ${configPath()} does not exist. ` +
        `Create it with \`nt project link ${name} --profile <env> --token nt_push_…\`.`,
    );
  }

  const projects = isRecord(config.projects) ? config.projects : {};
  // Object.hasOwn so prototype names ('toString' / 'hasOwnProperty') don't
  // slip past the missing-entry guard via the prototype chain.
  if (!Object.hasOwn(projects, name)) {
    const available = Object.keys(projects);
    const availableHint =
      available.length > 0 ? ` Registered projects: ${available.join(', ')}.` : '';
    throw new ProjectNotRegisteredError(
      `project "${name}" not registered in ${configPath()}.${availableHint}`,
    );
  }

  const entry: unknown = projects[name];
  if (!isProjectEntry(entry)) {
    const e = isRecord(entry) ? entry : {};
    const missing: string[] = [];
    if (typeof e['profile'] !== 'string') missing.push('profile');
    if (typeof e['pushToken'] !== 'string') missing.push('pushToken');
    throw new Error(
      `project "${name}" entry in ${configPath()} is malformed: missing ${missing.join(', ')}.`,
    );
  }

  const profiles = isRecord(config.profiles) ? config.profiles : {};
  // No `profile === undefined ||` guard needed — isProfileEntry returns
  // false for undefined, so the single check covers both "missing" and
  // "present but malformed" cases. Error message distinguishes them
  // by inspecting Object.hasOwn.
  const profile: unknown = profiles[entry.profile];
  if (!isProfileEntry(profile)) {
    if (!Object.hasOwn(profiles, entry.profile)) {
      throw new Error(
        `project "${name}" references profile "${entry.profile}" but that profile is not defined ` +
          `in ${configPath()}.`,
      );
    }
    throw new Error(
      `project "${name}" references profile "${entry.profile}" but that profile entry is ` +
        `malformed in ${configPath()} (apiUrl and authUrl must both be strings).`,
    );
  }

  return {
    token: entry.pushToken,
    apiUrl: profile.apiUrl,
    authUrl: profile.authUrl,
  };
}

export function clientForProject(name: string, overrides?: Partial<ClientOptions>): Client {
  const auth = resolveProjectAuth(name);
  return new Client({
    baseUrl: auth.apiUrl,
    token: auth.token,
    ...overrides,
  });
}
