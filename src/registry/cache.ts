import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join, parse, resolve } from 'node:path';
import { createHash } from 'node:crypto';
import { z } from 'zod';
import { eventTypeSpecSchema } from './client.js';

const CACHE_VERSION = 1 as const;
const CACHE_DIR_NAME = '.cache';
const NOTICKETS_DIR_NAME = '.notickets';

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

function findAncestorNoticketsDir(start: string): string | null {
  let current = resolve(start);
  // parse(current).root is filesystem root — we walk until we hit it.
  const root = parse(current).root;
  while (true) {
    if (existsSync(join(current, NOTICKETS_DIR_NAME))) return current;
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
  if (!existsSync(path)) return null;
  let raw: string;
  try {
    raw = readFileSync(path, 'utf-8');
  } catch {
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
  const tempPath = `${path}.${process.pid}.${Date.now()}.tmp`;
  writeFileSync(tempPath, JSON.stringify(validated), { mode: 0o600 });
  try {
    renameSync(tempPath, path);
  } catch (err) {
    try {
      unlinkSync(tempPath);
    } catch {
      // best-effort cleanup
    }
    throw err;
  }
}
