import { describe, it, expect, vi, beforeEach } from 'vitest';
import { createMcpServer } from '../create-server.js';

vi.mock('@modelcontextprotocol/sdk/server/mcp.js', () => {
  const registerTool = vi.fn();
  const connect = vi.fn().mockResolvedValue(undefined);
  const McpServer = vi.fn().mockImplementation(() => ({
    registerTool,
    connect,
  }));
  return { McpServer };
});

vi.mock('@modelcontextprotocol/sdk/server/stdio.js', () => {
  const StdioServerTransport = vi.fn();
  return { StdioServerTransport };
});

beforeEach(() => {
  vi.clearAllMocks();
});

describe('createMcpServer', () => {
  it('creates an McpServer with correct name and version', async () => {
    const { McpServer } = await import('@modelcontextprotocol/sdk/server/mcp.js');

    createMcpServer();

    expect(McpServer).toHaveBeenCalledOnce();
    const [serverInfo] = vi.mocked(McpServer).mock.calls[0]!;
    expect(serverInfo).toEqual(
      expect.objectContaining({
        name: 'no-tickets',
        version: expect.stringMatching(/^\d+\.\d+\.\d+/) as string,
      }),
    );
  });

  it('registers all expected tools', () => {
    const server = createMcpServer();
    const registerTool = vi.mocked(server.registerTool);

    const registeredNames = registerTool.mock.calls.map((call) => call[0]);

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
    const registerTool = vi.mocked(server.registerTool);

    for (const call of registerTool.mock.calls) {
      const [name, config] = call;
      expect(config).toHaveProperty('description');
      expect(typeof (config as Record<string, unknown>).description).toBe('string');
      expect((config as Record<string, unknown>).description).not.toBe('');
      expect(config).toHaveProperty('inputSchema');
      // inputSchema should be defined (Zod schema shape)
      expect((config as Record<string, unknown>).inputSchema).toBeDefined();
      expect(name).toBeTruthy();
    }
  });

  it('registers each tool with a callback function', () => {
    const server = createMcpServer();
    const registerTool = vi.mocked(server.registerTool);

    for (const call of registerTool.mock.calls) {
      const callback = call[2];
      expect(typeof callback).toBe('function');
    }
  });
});
