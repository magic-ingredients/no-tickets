import { createRequire } from 'node:module';
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';
import { toolError } from './tools/types.js';
import { handlePush } from './tools/push.js';
import { handleValidate } from './tools/validate.js';
import { handleStatus } from './tools/status.js';

const SERVER_NAME = 'no-tickets';

const require = createRequire(import.meta.url);
const { version: SERVER_VERSION } = require('../../package.json') as { version: string };

export function createMcpServer(): McpServer {
  const server = new McpServer(
    { name: SERVER_NAME, version: SERVER_VERSION },
    { capabilities: { tools: {} } },
  );

  server.registerTool(
    'push',
    {
      description: 'Push a v2 event payload to the no-tickets server',
      inputSchema: { payload: z.string().describe('JSON-encoded Push payload') },
    },
    async (args: { payload: string }) => {
      try {
        return await handlePush(args.payload);
      } catch (err) {
        return toolError(err);
      }
    },
  );

  server.registerTool(
    'validate',
    {
      description: 'Validate .notickets/ files against the format spec',
      inputSchema: { directory: z.string().optional().describe('Path to .notickets/ directory (defaults to .notickets)') },
    },
    async (args: { directory?: string }) => {
      try {
        return await handleValidate(args.directory);
      } catch (err) {
        return toolError(err);
      }
    },
  );

  server.registerTool(
    'status',
    {
      description: 'Check authentication and connection status',
      inputSchema: { verbose: z.boolean().optional().describe('Include detailed connection info') },
    },
    async () => {
      try {
        return handleStatus();
      } catch (err) {
        return toolError(err);
      }
    },
  );

  return server;
}

export async function startMcpServer(): Promise<void> {
  const server = createMcpServer();
  const transport = new StdioServerTransport();
  await server.connect(transport);
}
