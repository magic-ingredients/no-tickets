import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest';
import { createMcpServer } from '../create-server.js';

const constructorArgs: unknown[][] = [];

vi.mock('@modelcontextprotocol/sdk/server/mcp.js', () => {
  const registerTool = vi.fn();
  const connect = vi.fn().mockResolvedValue(undefined);
  class McpServer {
    registerTool = registerTool;
    connect = connect;
    constructor(...args: unknown[]) {
      constructorArgs.push(args);
    }
  }
  return { McpServer };
});

vi.mock('@modelcontextprotocol/sdk/server/stdio.js', () => {
  class StdioServerTransport {}
  return { StdioServerTransport };
});

beforeEach(() => {
  vi.clearAllMocks();
  constructorArgs.length = 0;
});

describe('createMcpServer', () => {
  it('creates an McpServer with correct name and version', () => {
    createMcpServer();

    expect(constructorArgs).toHaveLength(1);
    const [serverInfo] = constructorArgs[0]! as [Record<string, string>];
    expect(serverInfo).toEqual(
      expect.objectContaining({
        name: 'no-tickets',
        version: expect.stringMatching(/^\d+\.\d+\.\d+/) as string,
      }),
    );
  });

  it('registers all expected tools', () => {
    const server = createMcpServer();
    const registerTool = server.registerTool as unknown as Mock;

    const registeredNames = registerTool.mock.calls.map((call: unknown[]) => call[0]);

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

  it('registers each tool with a description and inputSchema', () => {
    const server = createMcpServer();
    const registerTool = server.registerTool as unknown as Mock;

    for (const call of registerTool.mock.calls as unknown[][]) {
      const [name, config] = call;
      expect(config).toHaveProperty('description');
      expect(typeof (config as Record<string, unknown>).description).toBe('string');
      expect((config as Record<string, unknown>).description).not.toBe('');
      expect(config).toHaveProperty('inputSchema');
      expect((config as Record<string, unknown>).inputSchema).toBeDefined();
      expect(name).toBeTruthy();
    }
  });

  it('registers each tool with a callback function', () => {
    const server = createMcpServer();
    const registerTool = server.registerTool as unknown as Mock;

    for (const call of registerTool.mock.calls as unknown[][]) {
      const callback = call[2];
      expect(typeof callback).toBe('function');
    }
  });
});
