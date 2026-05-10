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

/** Hard-fail error class for corrupt config.json — caller surfaces a
 *  user-friendly message and aborts. We do NOT want a write path that
 *  rebuilds-from-empty over a corrupt file: that's silent data loss
 *  on a secret-bearing file. */
export class ConfigCorruptError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'ConfigCorruptError';
  }
}

export function readConfigSync(): { config: ConfigShape; exists: boolean } {
  const file = configPath();
  if (!fs.existsSync(file)) return { config: {}, exists: false };
  const raw = fs.readFileSync(file, 'utf-8');
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (err) {
    throw new ConfigCorruptError(
      `${file} contains invalid JSON: ${err instanceof Error ? err.message : String(err)}. ` +
        `Refusing to proceed — fix the file by hand to avoid losing your profiles or push tokens.`,
    );
  }
  if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
    throw new ConfigCorruptError(
      `${file} root is not an object. ` +
        `Refusing to proceed — fix the file by hand to avoid losing your profiles or push tokens.`,
    );
  }
  return { config: parsed as ConfigShape, exists: true };
}

/** Atomic-ish write: rename a sibling tmp file over the destination so a
 *  partial write can't corrupt config.json mid-update. The tmp file is
 *  created with mode 0600; rename replaces the inode, so the new file
 *  inherits that mode regardless of what was there before. */
export function writeConfigSync(config: ConfigShape): void {
  const dir = configDir();
  fs.mkdirSync(dir, { recursive: true });
  const file = configPath();
  const tmp = `${file}.tmp.${process.pid}`;
  const json = `${JSON.stringify(config, null, 2)}\n`;
  fs.writeFileSync(tmp, json, { mode: 0o600 });
  fs.renameSync(tmp, file);
}

/** Mask a push token for human-readable output. Returns prefix + last 4
 *  characters separated by an ellipsis. Tokens shorter than the
 *  nt_push_ prefix length are unrealistic in practice (the registered
 *  format is nt_push_<64-hex>), but we don't crash on them — collapse
 *  to a tight ellipsis form. */
export function maskToken(token: string): string {
  const PREFIX = 'nt_push_';
  if (token.startsWith(PREFIX) && token.length > PREFIX.length + 4) {
    return `${PREFIX}…${token.slice(-4)}`;
  }
  // Fallback for unexpected formats — never expose more than the first 4
  // and last 2 characters.
  if (token.length <= 6) return token; // too short to mask meaningfully
  return `${token.slice(0, 4)}…${token.slice(-2)}`;
}
