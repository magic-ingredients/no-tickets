import type { z } from 'zod';

/** Shared shape for the registry/transport MCP tools registered in
 *  create-server.ts. inputSchema is a record of zod schemas keyed by field
 *  name (matches the @modelcontextprotocol/sdk's registerTool API). The
 *  handler is wired separately by the server (task 5-2) so this descriptor
 *  stays decoupled from the Client type. */
export interface ToolDescriptor {
  readonly name: string;
  readonly description: string;
  readonly inputSchema: Record<string, z.ZodTypeAny>;
}
