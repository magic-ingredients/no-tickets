import { describe, it, expect, vi } from 'vitest';
import { createEpicHandler, createFeatureHandler, createFixHandler } from '../tools/creation.js';
import { mockApiClient } from './mock-api-client.js';

describe('createEpicHandler', () => {
  it('calls apiClient.createEpic with params', async () => {
    const createEpic = vi.fn().mockResolvedValue({ id: 'epic-1' });
    const client = mockApiClient({ createEpic });

    await createEpicHandler({ projectId: 'p1', title: 'My Epic', description: 'Desc' }, client);

    expect(createEpic).toHaveBeenCalledWith({ projectId: 'p1', title: 'My Epic', description: 'Desc' });
  });

  it('returns MCP content with JSON-stringified result', async () => {
    const client = mockApiClient({
      createEpic: vi.fn().mockResolvedValue({ id: 'epic-1', title: 'My Epic' }),
    });

    const result = await createEpicHandler({ projectId: 'p1', title: 'My Epic' }, client);

    expect(result.content).toHaveLength(1);
    expect(result.content[0]!.type).toBe('text');
    const parsed: unknown = JSON.parse(result.content[0]!.text);
    expect(parsed).toEqual({ id: 'epic-1', title: 'My Epic' });
  });

  it('returns error content when API call fails', async () => {
    const client = mockApiClient({
      createEpic: vi.fn().mockRejectedValue(new Error('400: Bad request')),
    });

    const result = await createEpicHandler({ projectId: 'p1', title: '' }, client);

    expect(result.isError).toBe(true);
    expect(result.content[0]!.text).toContain('400');
  });
});

describe('createFeatureHandler', () => {
  it('calls apiClient.createFeature with params', async () => {
    const createFeature = vi.fn().mockResolvedValue({ id: 'feat-1' });
    const client = mockApiClient({ createFeature });

    await createFeatureHandler(
      { projectId: 'p1', epicId: 'e1', title: 'Feature', description: 'Desc' },
      client,
    );

    expect(createFeature).toHaveBeenCalledWith({
      projectId: 'p1', epicId: 'e1', title: 'Feature', description: 'Desc',
    });
  });

  it('returns MCP content with JSON-stringified result', async () => {
    const client = mockApiClient({
      createFeature: vi.fn().mockResolvedValue({ id: 'feat-1' }),
    });

    const result = await createFeatureHandler(
      { projectId: 'p1', epicId: 'e1', title: 'Feature' },
      client,
    );

    expect(result.content).toHaveLength(1);
    const parsed: unknown = JSON.parse(result.content[0]!.text);
    expect(parsed).toEqual({ id: 'feat-1' });
  });

  it('returns error content when API call fails', async () => {
    const client = mockApiClient({
      createFeature: vi.fn().mockRejectedValue(new Error('500: Server error')),
    });

    const result = await createFeatureHandler(
      { projectId: 'p1', epicId: 'e1', title: 'Feature' },
      client,
    );

    expect(result.isError).toBe(true);
    expect(result.content[0]!.text).toContain('500');
  });
});

describe('createFixHandler', () => {
  it('calls apiClient.createFix with params', async () => {
    const createFix = vi.fn().mockResolvedValue({ id: 'fix-1' });
    const client = mockApiClient({ createFix });

    await createFixHandler(
      { projectId: 'p1', epicId: 'e1', title: 'Bug', description: 'Broken' },
      client,
    );

    expect(createFix).toHaveBeenCalledWith({
      projectId: 'p1', epicId: 'e1', title: 'Bug', description: 'Broken',
    });
  });

  it('returns MCP content with JSON-stringified result', async () => {
    const client = mockApiClient({
      createFix: vi.fn().mockResolvedValue({ id: 'fix-1' }),
    });

    const result = await createFixHandler(
      { projectId: 'p1', epicId: 'e1', title: 'Bug' },
      client,
    );

    expect(result.content).toHaveLength(1);
    const parsed: unknown = JSON.parse(result.content[0]!.text);
    expect(parsed).toEqual({ id: 'fix-1' });
  });

  it('returns error content when API call fails', async () => {
    const client = mockApiClient({
      createFix: vi.fn().mockRejectedValue(new Error('401: Unauthorized')),
    });

    const result = await createFixHandler(
      { projectId: 'p1', epicId: 'e1', title: 'Bug' },
      client,
    );

    expect(result.isError).toBe(true);
    expect(result.content[0]!.text).toContain('401');
  });
});

describe('toolError non-Error handling', () => {
  it('converts non-Error rejection to string', async () => {
    const client = mockApiClient({
      createEpic: vi.fn().mockRejectedValue('plain string error'),
    });

    const result = await createEpicHandler({ projectId: 'p1', title: 'Epic' }, client);

    expect(result.isError).toBe(true);
    expect(result.content[0]!.text).toBe('plain string error');
  });
});
