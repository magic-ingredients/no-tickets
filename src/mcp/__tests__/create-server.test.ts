import { describe, it, expect, vi, beforeEach } from 'vitest';
import { createMcpServer, startMcpServer } from '../create-server.js';

interface MockServerInstance {
  readonly constructorArgs: unknown[];
  readonly registerToolCalls: Array<{ name: string; config: Record<string, unknown>; callback: unknown }>;
  readonly connectCalls: unknown[];
}

let lastInstance: MockServerInstance | undefined;

vi.mock('@modelcontextprotocol/sdk/server/mcp.js', () => {
  class McpServer {
    private _calls: MockServerInstance;

    constructor(...args: unknown[]) {
      this._calls = {
        constructorArgs: args,
        registerToolCalls: [],
        connectCalls: [],
      };
      lastInstance = this._calls;
    }

    registerTool(name: string, config: Record<string, unknown>, callback: unknown) {
      this._calls.registerToolCalls.push({ name, config, callback });
    }

    async connect(transport: unknown) {
      this._calls.connectCalls.push(transport);
    }
  }
  return { McpServer };
});

vi.mock('@modelcontextprotocol/sdk/server/stdio.js', () => {
  class StdioServerTransport {}
  return { StdioServerTransport };
});

beforeEach(() => {
  lastInstance = undefined;
});

function findTool(name: string) {
  const tool = lastInstance!.registerToolCalls.find((c) => c.name === name);
  expect(tool).toBeDefined();
  return tool!;
}

describe('createMcpServer', () => {
  it('creates an McpServer with correct name and version', () => {
    createMcpServer();

    expect(lastInstance).toBeDefined();
    const [serverInfo] = lastInstance!.constructorArgs as [Record<string, string>];
    expect(serverInfo.name).toBe('no-tickets');
    expect(serverInfo.version).toMatch(/^\d+\.\d+\.\d+/);
  });

  it('passes version from package.json', () => {
    createMcpServer();

    const [serverInfo] = lastInstance!.constructorArgs as [Record<string, string>];
    expect(serverInfo.version).toBe('2.0.0');
  });

  it('passes tools capability in options', () => {
    createMcpServer();

    const [, options] = lastInstance!.constructorArgs as [unknown, { capabilities: Record<string, unknown> }];
    expect(options.capabilities).toHaveProperty('tools');
  });

  it('registers all expected tools', () => {
    createMcpServer();

    const registeredNames = lastInstance!.registerToolCalls.map((c) => c.name);

    const expectedTools = [
      'create_epic',
      'create_feature',
      'create_fix',
      'break_down',
      'list_board',
      'update_feature',
      'move_to_phase',
      'assign',
      'list_feed',
      'get_template',
    ];

    for (const tool of expectedTools) {
      expect(registeredNames).toContain(tool);
    }
    expect(registeredNames).toHaveLength(expectedTools.length);
  });

  it('registers each tool with a non-empty description and inputSchema', () => {
    createMcpServer();

    for (const { name, config } of lastInstance!.registerToolCalls) {
      expect(name).toBeTruthy();
      expect(typeof config.description).toBe('string');
      expect(config.description).not.toBe('');
      expect(config.inputSchema).toBeDefined();
      expect(typeof config.inputSchema).toBe('object');
      expect(Object.keys(config.inputSchema as object).length).toBeGreaterThan(0);
    }
  });

  it('registers each tool with a callback function', () => {
    createMcpServer();

    for (const { callback } of lastInstance!.registerToolCalls) {
      expect(typeof callback).toBe('function');
    }
  });

  it('stub callbacks return not-yet-implemented content', async () => {
    createMcpServer();

    const { callback } = lastInstance!.registerToolCalls[0]!;
    const result = await (callback as () => Promise<unknown>)();
    expect(result).toEqual({
      content: [{ type: 'text', text: 'Not yet implemented' }],
    });
  });

  describe('tool input schemas', () => {
    beforeEach(() => {
      createMcpServer();
    });

    it('list_board requires projectId', () => {
      const schema = findTool('list_board').config.inputSchema as Record<string, unknown>;
      expect(schema).toHaveProperty('projectId');
    });

    it('list_feed requires projectId', () => {
      const schema = findTool('list_feed').config.inputSchema as Record<string, unknown>;
      expect(schema).toHaveProperty('projectId');
    });

    it('create_epic requires projectId and title', () => {
      const schema = findTool('create_epic').config.inputSchema as Record<string, unknown>;
      expect(schema).toHaveProperty('projectId');
      expect(schema).toHaveProperty('title');
      expect(schema).toHaveProperty('description');
    });

    it('create_feature requires projectId, epicId, and title', () => {
      const schema = findTool('create_feature').config.inputSchema as Record<string, unknown>;
      expect(schema).toHaveProperty('projectId');
      expect(schema).toHaveProperty('epicId');
      expect(schema).toHaveProperty('title');
      expect(schema).toHaveProperty('description');
    });

    it('create_fix requires projectId, epicId, and title', () => {
      const schema = findTool('create_fix').config.inputSchema as Record<string, unknown>;
      expect(schema).toHaveProperty('projectId');
      expect(schema).toHaveProperty('epicId');
      expect(schema).toHaveProperty('title');
      expect(schema).toHaveProperty('description');
    });

    it('update_feature requires projectId and featureId', () => {
      const schema = findTool('update_feature').config.inputSchema as Record<string, unknown>;
      expect(schema).toHaveProperty('projectId');
      expect(schema).toHaveProperty('featureId');
      expect(schema).toHaveProperty('title');
      expect(schema).toHaveProperty('description');
    });

    it('move_to_phase requires projectId, featureId, and phase', () => {
      const schema = findTool('move_to_phase').config.inputSchema as Record<string, unknown>;
      expect(schema).toHaveProperty('projectId');
      expect(schema).toHaveProperty('featureId');
      expect(schema).toHaveProperty('phase');
    });

    it('assign requires projectId, featureId, assignee, and assigneeType', () => {
      const schema = findTool('assign').config.inputSchema as Record<string, unknown>;
      expect(schema).toHaveProperty('projectId');
      expect(schema).toHaveProperty('featureId');
      expect(schema).toHaveProperty('assignee');
      expect(schema).toHaveProperty('assigneeType');
    });

    it('get_template requires templateType', () => {
      const schema = findTool('get_template').config.inputSchema as Record<string, unknown>;
      expect(schema).toHaveProperty('templateType');
    });

    it('break_down requires projectId and featureId', () => {
      const schema = findTool('break_down').config.inputSchema as Record<string, unknown>;
      expect(schema).toHaveProperty('projectId');
      expect(schema).toHaveProperty('featureId');
      expect(schema).toHaveProperty('context');
    });
  });
});

describe('startMcpServer', () => {
  it('creates server, connects with StdioServerTransport', async () => {
    await startMcpServer();

    expect(lastInstance).toBeDefined();
    expect(lastInstance!.connectCalls).toHaveLength(1);
    const [transport] = lastInstance!.connectCalls;
    expect(transport).toBeDefined();
  });
});
