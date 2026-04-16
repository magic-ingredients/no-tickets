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

  it('registers each tool with a description and inputSchema', () => {
    createMcpServer();

    for (const { name, config } of lastInstance!.registerToolCalls) {
      expect(name).toBeTruthy();
      expect(typeof config.description).toBe('string');
      expect(config.description).not.toBe('');
      expect(config.inputSchema).toBeDefined();
    }
  });

  it('registers each tool with a callback function', () => {
    createMcpServer();

    for (const { callback } of lastInstance!.registerToolCalls) {
      expect(typeof callback).toBe('function');
    }
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
