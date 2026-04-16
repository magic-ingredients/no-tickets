import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';

const SERVER_NAME = 'no-tickets';
const SERVER_VERSION = '2.0.0';

function stubHandler(): Promise<{ content: Array<{ type: 'text'; text: string }> }> {
  return Promise.resolve({
    content: [{ type: 'text' as const, text: 'Not yet implemented' }],
  });
}

interface ToolDef {
  readonly name: string;
  readonly description: string;
  readonly inputSchema: Record<string, z.ZodType>;
}

const toolDefs: readonly ToolDef[] = [
  {
    name: 'list_board',
    description: 'List the project board with features grouped by phase',
    inputSchema: { projectId: z.string().describe('The project ID') },
  },
  {
    name: 'list_feed',
    description: 'List recent activity events for a project',
    inputSchema: { projectId: z.string().describe('The project ID') },
  },
  {
    name: 'create_epic',
    description: 'Create a new epic in a project',
    inputSchema: {
      projectId: z.string().describe('The project ID'),
      title: z.string().describe('Epic title'),
      description: z.string().optional().describe('Epic description'),
    },
  },
  {
    name: 'create_feature',
    description: 'Create a new feature under an epic',
    inputSchema: {
      projectId: z.string().describe('The project ID'),
      epicId: z.string().describe('The parent epic ID'),
      title: z.string().describe('Feature title'),
      description: z.string().optional().describe('Feature description'),
    },
  },
  {
    name: 'create_fix',
    description: 'Create a new fix (bug report) under an epic',
    inputSchema: {
      projectId: z.string().describe('The project ID'),
      epicId: z.string().describe('The parent epic ID'),
      title: z.string().describe('Fix title'),
      description: z.string().optional().describe('Fix description'),
    },
  },
  {
    name: 'update_feature',
    description: 'Update a feature title, description, or tasks',
    inputSchema: {
      projectId: z.string().describe('The project ID'),
      featureId: z.string().describe('The feature ID to update'),
      title: z.string().optional().describe('New title'),
      description: z.string().optional().describe('New description'),
    },
  },
  {
    name: 'move_to_phase',
    description: 'Move a feature to a different workflow phase',
    inputSchema: {
      projectId: z.string().describe('The project ID'),
      featureId: z.string().describe('The feature ID'),
      phase: z.string().describe('Target phase (ideation, development, testing, review, done)'),
    },
  },
  {
    name: 'assign',
    description: 'Assign a human or agent to a feature',
    inputSchema: {
      projectId: z.string().describe('The project ID'),
      featureId: z.string().describe('The feature ID'),
      assignee: z.string().describe('Assignee name'),
      assigneeType: z.enum(['human', 'agent']).describe('Whether the assignee is a human or an agent'),
    },
  },
  {
    name: 'get_template',
    description: 'Get a markdown template for creating an epic, feature, or fix',
    inputSchema: {
      templateType: z.enum(['epic', 'feature', 'fix']).describe('The type of template to return'),
    },
  },
  {
    name: 'break_down',
    description: 'Break down a feature into tasks using server-side LLM decomposition',
    inputSchema: {
      projectId: z.string().describe('The project ID'),
      featureId: z.string().describe('The feature ID to break down'),
      context: z.string().optional().describe('Additional context for the breakdown'),
    },
  },
];

export function createMcpServer(): McpServer {
  const server = new McpServer(
    { name: SERVER_NAME, version: SERVER_VERSION },
    { capabilities: { tools: {} } },
  );

  for (const def of toolDefs) {
    server.registerTool(
      def.name,
      { description: def.description, inputSchema: def.inputSchema },
      stubHandler,
    );
  }

  return server;
}

export async function startMcpServer(): Promise<void> {
  const server = createMcpServer();
  const transport = new StdioServerTransport();
  await server.connect(transport);
}
