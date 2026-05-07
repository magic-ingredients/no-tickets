import {
  mkdirSync,
  readFileSync,
  renameSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join, parse, resolve } from 'node:path';
import { createHash, randomBytes } from 'node:crypto';
import { z } from 'zod';
import { eventTypeSpecSchema } from './client.js';

const CACHE_VERSION = 1 as const;
const CACHE_DIR_NAME = '.cache';
const NOTICKETS_DIR_NAME = '.notickets';
const GIT_DIR_NAME = '.git';

export const cacheFileSchema = z.object({
  version: z.literal(CACHE_VERSION),
  etag: z.string().min(1),
  fetchedAt: z.string().datetime(),
  serverUrl: z.string().min(1),
  types: z.array(eventTypeSpecSchema),
});

export type CacheFile = Readonly<z.infer<typeof cacheFileSchema>>;

function homeBase(): string {
  return process.env['NO_TICKETS_HOME'] ?? homedir();
}

function isDirectory(p: string): boolean {
  try {
    return statSync(p).isDirectory();
  } catch {
    return false;
  }
}

// Walk from `start` upwards looking for `.notickets/`. Stop at the first
// ancestor that contains either a `.notickets/` directory (use it) or a
// `.git/` directory (use user-local fallback — per PRD: "walks from cwd up
// to git root looking for .notickets/"). A stray `~/.notickets/` therefore
// cannot capture caches for unrelated projects.
function findAncestorNoticketsDir(start: string): string | null {
  let current = resolve(start);
  const root = parse(current).root;
  while (true) {
    if (isDirectory(join(current, NOTICKETS_DIR_NAME))) return current;
    if (isDirectory(join(current, GIT_DIR_NAME))) return null;
    if (current === root) return null;
    current = dirname(current);
  }
}

function hashUrl(serverUrl: string): string {
  return createHash('sha256').update(serverUrl).digest('hex').slice(0, 16);
}

export function cachePath(serverUrl: string): string {
  const ancestor = findAncestorNoticketsDir(process.cwd());
  const base = ancestor ?? homeBase();
  return join(base, NOTICKETS_DIR_NAME, CACHE_DIR_NAME, `registry-${hashUrl(serverUrl)}.json`);
}

export function readCache(serverUrl: string): CacheFile | null {
  const path = cachePath(serverUrl);
  let raw: string;
  try {
    raw = readFileSync(path, 'utf-8');
  } catch {
    // ENOENT (missing) or EACCES (unreadable) — caller treats as cache miss.
    return null;
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }
  const result = cacheFileSchema.safeParse(parsed);
  return result.success ? result.data : null;
}

export function writeCache(serverUrl: string, file: CacheFile): void {
  const validated = cacheFileSchema.parse(file);
  const path = cachePath(serverUrl);
  mkdirSync(dirname(path), { recursive: true });
  const tempPath = `${path}.${randomBytes(8).toString('hex')}.tmp`;
  writeFileSync(tempPath, JSON.stringify(validated), { mode: 0o600 });
  renameSync(tempPath, path);
}
