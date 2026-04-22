import { readdir, readFile, stat } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import type { FileEntry } from './types.js';

/**
 * Read all .md files from a directory (one level of subdirectories).
 * Parallelizes I/O with Promise.all. Validates path is within cwd.
 */
export async function readNoTicketsDir(dir: string): Promise<readonly FileEntry[]> {
  const resolved = resolve(dir);
  const cwd = resolve(process.cwd());
  if (!resolved.startsWith(cwd)) {
    throw new Error(`Path "${dir}" is outside the current working directory`);
  }

  let items: string[];
  try {
    items = await readdir(resolved);
  } catch {
    return [];
  }

  const itemPaths = items.map((item) => join(resolved, item));
  const stats = await Promise.all(
    itemPaths.map((p) => stat(p).catch(() => null)),
  );

  const entries: FileEntry[] = [];
  const subDirReads: Promise<FileEntry[]>[] = [];

  for (let i = 0; i < items.length; i++) {
    const itemStat = stats[i];
    const itemPath = itemPaths[i]!;
    const item = items[i]!;

    if (itemStat == null) continue;

    if (itemStat.isFile() && item.endsWith('.md')) {
      entries.push({ path: itemPath, content: '' });
    } else if (itemStat.isDirectory()) {
      subDirReads.push(readSubDir(itemPath));
    }
  }

  const topLevelContents = await Promise.all(
    entries.map(async (entry) => {
      const content = await readFile(entry.path, 'utf-8');
      return { path: entry.path, content };
    }),
  );

  const subDirResults = await Promise.all(subDirReads);

  return [...topLevelContents, ...subDirResults.flat()];
}

async function readSubDir(dirPath: string): Promise<FileEntry[]> {
  let subItems: string[];
  try {
    subItems = await readdir(dirPath);
  } catch {
    return [];
  }

  const mdFiles = subItems.filter((name) => name.endsWith('.md'));
  const results = await Promise.all(
    mdFiles.map(async (name) => {
      const filePath = join(dirPath, name);
      const content = await readFile(filePath, 'utf-8').catch(() => null);
      if (content === null) return null;
      return { path: filePath, content };
    }),
  );

  return results.filter((r): r is FileEntry => r !== null);
}
