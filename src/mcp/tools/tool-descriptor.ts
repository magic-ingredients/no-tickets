import type { z } from 'zod';

/** Shared shape for the registry/transport MCP tools registered in
 *  create-server.ts. inputSchema is a record of zod schemas keyed by field
 *  name (matches the @modelcontextprotocol/sdk's registerTool API). The
 *  handler signature is intentionally generic — concrete tools narrow it
 *  via per-tool type guards. */
export interface ToolDescriptor {
  readonly name: string;
  readonly description: string;
  readonly inputSchema: Record<string, z.ZodTypeAny>;
}
