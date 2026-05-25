import { readdir, readFile, stat } from 'node:fs/promises';
import { join, resolve, sep } from 'node:path';
import type { FileEntry } from './types.js';

/**
 * Read all .md files from a directory (one level of subdirectories).
 * Parallelizes I/O with Promise.all. Validates path is within cwd.
 */
export async function readNoTicketsDir(dir: string): Promise<readonly FileEntry[]> {
  const resolved = resolve(dir);
  const cwd = resolve(process.cwd());
  if (resolved !== cwd && !resolved.startsWith(cwd + sep)) {
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

  const mdPaths: string[] = [];
  const subDirReads: Promise<FileEntry[]>[] = [];

  for (let i = 0; i < items.length; i++) {
    const itemStat = stats[i];
    const itemPath = itemPaths[i]!;
    const item = items[i]!;

    if (itemStat == null) continue;

    if (itemStat.isFile() && item.endsWith('.md')) {
      mdPaths.push(itemPath);
    } else if (itemStat.isDirectory()) {
      subDirReads.push(readSubDir(itemPath));
    }
  }

  const topLevelFiles = await Promise.all(
    mdPaths.map(async (filePath) => {
      const content = await readFile(filePath, 'utf-8').catch(() => null);
      if (content === null) return null;
      return { path: filePath, content };
    }),
  );

  const subDirResults = await Promise.all(subDirReads);

  return [
    ...topLevelFiles.filter((f): f is FileEntry => f !== null),
    ...subDirResults.flat(),
  ];
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
