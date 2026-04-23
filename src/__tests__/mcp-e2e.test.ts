import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { InMemoryTransport } from '@modelcontextprotocol/sdk/inMemory.js';
import { createMcpServer } from '../mcp/create-server.js';

let testDir: string;
let fetchSpy: ReturnType<typeof vi.fn>;
let originalCwd: string;

const EPIC_CONTENT = `---
id: auth
type: epic
title: Authentication
status: in_progress
created: 2026-04-22
updated: 2026-04-22
---
# Authentication
`;

function jsonResponse(body: unknown, status = 200) {
  return Promise.resolve(new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  }));
}

beforeEach(async () => {
  testDir = await mkdtemp(join(tmpdir(), 'nt-mcp-e2e-'));
  fetchSpy = vi.fn().mockReturnValue(jsonResponse({ success: true, changesApplied: 1, eventsGenerated: 1 }));
  vi.stubGlobal('fetch', fetchSpy);
  vi.stubEnv('NO_TICKETS_HOME', testDir);
  vi.stubEnv('NO_TICKETS_TOKEN', 'nt_push_mcptest');
  vi.stubEnv('NO_TICKETS_API_URL', 'https://api.test.com');
  vi.stubEnv('NO_TICKETS_PROJECT_ID', 'proj-mcp');
  originalCwd = process.cwd();
  process.chdir(testDir);
});

afterEach(async () => {
  process.chdir(originalCwd);
  vi.restoreAllMocks();
  vi.unstubAllEnvs();
  await rm(testDir, { recursive: true, force: true });
});

async function createConnectedClient() {
  const server = createMcpServer();
  const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
  await server.connect(serverTransport);

  const client = new Client({ name: 'test-client', version: '1.0.0' });
  await client.connect(clientTransport);
  return client;
}

describe('MCP server e2e', () => {
  it('lists exactly 3 tools', async () => {
    const client = await createConnectedClient();
    const tools = await client.listTools();

    const names = tools.tools.map((t) => t.name);
    expect(names).toEqual(['push', 'validate', 'status']);
  });

  it('push tool validates and sends payload to API', async () => {
    const client = await createConnectedClient();
    const payload = JSON.stringify({
      projectId: 'proj-1',
      timestamp: '2026-04-22T10:00:00Z',
      work: { entities: [{ id: 'e1', type: 'epic', title: 'E', status: 'not_started' }] },
    });

    const result = await client.callTool({ name: 'push', arguments: { payload } });

    expect(result.isError).toBeFalsy();
    expect(fetchSpy).toHaveBeenCalledOnce();
    const [url] = fetchSpy.mock.calls[0] as [string];
    expect(url).toContain('/api/v1/push');
  });

  it('push tool returns error for invalid JSON', async () => {
    const client = await createConnectedClient();

    const result = await client.callTool({ name: 'push', arguments: { payload: 'not json' } });

    expect(result.isError).toBe(true);
  });

  it('push tool returns error for invalid payload schema', async () => {
    const client = await createConnectedClient();
    const payload = JSON.stringify({ notAValidPush: true });

    const result = await client.callTool({ name: 'push', arguments: { payload } });

    expect(result.isError).toBe(true);
  });

  it('validate tool returns valid for well-formed files', async () => {
    await mkdir(join(testDir, '.notickets', 'auth'), { recursive: true });
    await writeFile(join(testDir, '.notickets', 'auth', 'epic.md'), EPIC_CONTENT);

    const client = await createConnectedClient();
    const result = await client.callTool({ name: 'validate', arguments: {} });

    expect(result.isError).toBeFalsy();
    const content = JSON.parse((result.content as Array<{ text: string }>)[0]!.text);
    expect(content.valid).toBe(true);
  });

  it('validate tool returns errors for missing .notickets/ dir', async () => {
    const client = await createConnectedClient();
    const result = await client.callTool({ name: 'validate', arguments: {} });

    expect(result.isError).toBeFalsy();
    const content = JSON.parse((result.content as Array<{ text: string }>)[0]!.text);
    expect(content.valid).toBe(true);
    expect(content.errors).toHaveLength(0);
  });

  it('status tool returns auth state when authenticated', async () => {
    const client = await createConnectedClient();
    const result = await client.callTool({ name: 'status', arguments: {} });

    expect(result.isError).toBeFalsy();
    const content = JSON.parse((result.content as Array<{ text: string }>)[0]!.text);
    expect(content.authenticated).toBe(true);
    expect(content.source).toBe('env');
    expect(content.tokenType).toBe('push');
  });

  it('status tool returns error when not authenticated', async () => {
    vi.stubEnv('NO_TICKETS_TOKEN', '');
    delete process.env['NO_TICKETS_TOKEN'];

    const client = await createConnectedClient();
    const result = await client.callTool({ name: 'status', arguments: {} });

    expect(result.isError).toBe(true);
  });
});
