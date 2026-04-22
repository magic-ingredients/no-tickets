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
    expect(serverInfo.version).toBe('2.0.0');
  });

  it('passes tools capability in options', () => {
    createMcpServer();

    const [, options] = lastInstance!.constructorArgs as [unknown, { capabilities: Record<string, unknown> }];
    expect(options.capabilities).toHaveProperty('tools');
  });

  it('registers exactly 3 tools: push, validate, status', () => {
    createMcpServer();

    const registeredNames = lastInstance!.registerToolCalls.map((c) => c.name);

    expect(registeredNames).toEqual(['push', 'validate', 'status']);
    expect(registeredNames).toHaveLength(3);
  });

  it('registers each tool with a non-empty description and inputSchema', () => {
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

  describe('tool input schemas', () => {
    beforeEach(() => {
      createMcpServer();
    });

    it('push accepts a payload object', () => {
      const schema = findTool('push').config.inputSchema as Record<string, unknown>;
      expect(schema).toHaveProperty('payload');
    });

    it('validate accepts optional directory parameter', () => {
      const schema = findTool('validate').config.inputSchema as Record<string, unknown>;
      expect(schema).toHaveProperty('directory');
    });

    it('status has empty input schema', () => {
      const schema = findTool('status').config.inputSchema as Record<string, unknown>;
      expect(Object.keys(schema)).toHaveLength(0);
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
