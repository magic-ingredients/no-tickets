import { createHash, randomBytes } from 'node:crypto';
import { hostname, homedir } from 'node:os';
import { mkdirSync, readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { type Source, SDK_VERSION } from './core/source.js';

// Read HOME/USERPROFILE env vars first (testable via env-stubbing) before
// falling back to os.homedir(). ESM bindings on os.homedir cannot be spied.
function resolveHome(): string {
  return process.env['HOME'] ?? process.env['USERPROFILE'] ?? homedir();
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
    // Pre-existing empty/whitespace file: overwrite (no 'wx' flag — race with
    // a concurrent first-run is not possible if a stale empty file already
    // exists from an earlier crash).
    mkdirSync(dir, { recursive: true });
    const salt = randomBytes(16).toString('hex');
    writeFileSync(path, salt, { mode: 0o600 });
    return salt;
  }
  mkdirSync(dir, { recursive: true });
  const salt = randomBytes(16).toString('hex');
  // Atomic create-or-fail: handles the race where two concurrent first-runs
  // both reach the write step. Loser re-reads the winner's salt.
  try {
    writeFileSync(path, salt, { mode: 0o600, flag: 'wx' });
    return salt;
  } catch {
    const winner = readFileSync(path, 'utf-8').trim();
    if (winner.length === 0) throw new Error('salt file present but empty after concurrent write');
    return winner;
  }
}

function hashedMachine(): string {
  const salt = readOrCreateMachineSalt();
  return createHash('sha256').update(`${salt}:${hostname()}`).digest('hex').slice(0, 16);
}

/**
 * Detect a Source for direct SDK use. Used by the transport (Feature 2) to
 * auto-fill source on every event when the caller doesn't provide one.
 *
 * - `name: 'sdk'` always. CI provenance is caller-driven now — surface
 *   handlers (CLI / MCP) override `name` to `'cli'` / `'mcp'`, and CI
 *   scripts that want `provider=github-actions` style attribution supply
 *   it explicitly via `PublishEvent.source.attributes` or the CLI's
 *   `--source-attribute key=value` flag.
 * - `attributes.machine` populated only when `NO_TICKETS_INCLUDE_MACHINE=1`.
 *   Value is a hashed hostname using a per-installation salt (never raw hostname).
 */
export function detectSource(): Source {
  const attributes: Record<string, string | number | boolean> = {};

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
    name: 'sdk',
    sdkVersion: SDK_VERSION,
  };

  if (Object.keys(attributes).length > 0) {
    return { ...source, attributes };
  }
  return source;
}
