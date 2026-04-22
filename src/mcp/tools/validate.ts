import { readNoTicketsDir } from '../../core/fs.js';
import { validateFiles } from '../../commands/validate.js';
import { toolSuccess, type ToolResult } from './types.js';

export async function handleValidate(directory?: string): Promise<ToolResult> {
  const dir = directory ?? '.notickets';
  const files = await readNoTicketsDir(dir);
  const result = validateFiles(files);
  return toolSuccess(result);
}
