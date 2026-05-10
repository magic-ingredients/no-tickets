// Shared file I/O for the project link/list/unlink commands.
//
// Reads and writes ~/.notickets/config.json, preserving any existing
// `profiles` section verbatim. File mode 0600 — config carries push
// tokens, treat as a secret.

import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';

export interface ProjectEntry {
  readonly profile: string;
  readonly pushToken: string;
}

export interface ConfigShape {
  readonly profiles?: Record<string, unknown>;
  readonly projects?: Record<string, ProjectEntry>;
  // Other top-level keys are preserved unchanged on rewrite.
  readonly [other: string]: unknown;
}

export function configDir(): string {
  const home = process.env['NO_TICKETS_HOME'] || os.homedir();
  return path.join(home, '.notickets');
}

export function configPath(): string {
  return path.join(configDir(), 'config.json');
}

export function readConfigSync(): { config: ConfigShape; exists: boolean } {
  const file = configPath();
  if (!fs.existsSync(file)) return { config: {}, exists: false };
  const raw = fs.readFileSync(file, 'utf-8');
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return { config: {}, exists: true };
  }
  if (typeof parsed !== 'object' || parsed === null) return { config: {}, exists: true };
  return { config: parsed as ConfigShape, exists: true };
}

/** Atomic-ish write: rename a sibling tmp file over the destination so a
 *  partial write can't corrupt config.json mid-update. mode 0600 is
 *  applied via writeFileSync's mode option AND a follow-up chmod (covers
 *  the case where the file already existed with a wider mode). */
export function writeConfigSync(config: ConfigShape): void {
  const dir = configDir();
  fs.mkdirSync(dir, { recursive: true });
  const file = configPath();
  const tmp = `${file}.tmp.${process.pid}`;
  const json = `${JSON.stringify(config, null, 2)}\n`;
  fs.writeFileSync(tmp, json, { mode: 0o600 });
  fs.renameSync(tmp, file);
  fs.chmodSync(file, 0o600);
}

export function maskToken(token: string): string {
  // nt_push_<sha-ish>: keep the prefix and the last 4 characters so the
  // user can disambiguate which token without exposing the full secret.
  if (token.length <= 12) return `${token.slice(0, 4)}…${token.slice(-2)}`;
  const last4 = token.slice(-4);
  const prefix = token.startsWith('nt_push_') ? 'nt_push_' : token.slice(0, 8);
  return `${prefix}…${last4}`;
}
