import { readdir, readFile, stat } from 'node:fs/promises';
import { join } from 'node:path';
import { validateFiles } from '../../commands/validate.js';
import { toolSuccess, type ToolResult } from './types.js';
import type { FileEntry } from '../../core/types.js';

async function readNoTicketsDir(dir: string): Promise<readonly FileEntry[]> {
  const entries: FileEntry[] = [];
  let items: string[];
  try {
    items = await readdir(dir);
  } catch {
    return [];
  }

  for (const item of items) {
    const itemPath = join(dir, item);
    const itemStat = await stat(itemPath).catch(() => null);
    if (itemStat === null) continue;

    if (itemStat.isFile() && item.endsWith('.md')) {
      const content = await readFile(itemPath, 'utf-8');
      entries.push({ path: itemPath, content });
    } else if (itemStat.isDirectory()) {
      const subItems = await readdir(itemPath).catch(() => [] as string[]);
      for (const subItem of subItems) {
        if (!subItem.endsWith('.md')) continue;
        const subPath = join(itemPath, subItem);
        const content = await readFile(subPath, 'utf-8').catch(() => null);
        if (content !== null) {
          entries.push({ path: subPath, content });
        }
      }
    }
  }

  return entries;
}

export async function handleValidate(directory?: string): Promise<ToolResult> {
  const dir = directory ?? '.notickets';
  const files = await readNoTicketsDir(dir);
  const result = validateFiles(files);
  return toolSuccess(result);
}
