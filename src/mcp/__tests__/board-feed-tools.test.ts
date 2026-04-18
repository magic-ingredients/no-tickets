import { describe, it, expect, vi } from 'vitest';
import { listBoardHandler, listFeedHandler } from '../tools/board-feed.js';
import { mockApiClient } from './mock-api-client.js';
import type { BoardState, FeedEvent } from '../../core/types.js';

describe('listBoardHandler', () => {
  it('calls apiClient.getBoard with projectId', async () => {
    const board: BoardState = { projectId: 'p1', columns: [] };
    const getBoard = vi.fn().mockResolvedValue(board);
    const client = mockApiClient({ getBoard });

    await listBoardHandler({ projectId: 'p1' }, client);

    expect(getBoard).toHaveBeenCalledWith('p1');
  });

  it('returns MCP content with JSON-stringified board', async () => {
    const board: BoardState = {
      projectId: 'p1',
      columns: [{
        phase: 'development',
        features: [{
          id: 'f1', epicId: 'e1', title: 'Feature 1', type: 'feature',
          phase: 'development', tasks: { total: 3, completed: 1 },
          tests: { total: 5, passing: 5 },
        }],
      }],
    };
    const client = mockApiClient({ getBoard: vi.fn().mockResolvedValue(board) });

    const result = await listBoardHandler({ projectId: 'p1' }, client);

    expect(result.content).toHaveLength(1);
    expect(result.content[0]!.type).toBe('text');
    const parsed: unknown = JSON.parse(result.content[0]!.text);
    expect(parsed).toEqual(board);
  });

  it('returns error content when API call fails', async () => {
    const client = mockApiClient({
      getBoard: vi.fn().mockRejectedValue(new Error('404: Not found')),
    });

    const result = await listBoardHandler({ projectId: 'bad' }, client);

    expect(result.isError).toBe(true);
    expect(result.content[0]!.text).toContain('404');
  });
});

describe('listFeedHandler', () => {
  it('calls apiClient.getFeed with projectId', async () => {
    const getFeed = vi.fn().mockResolvedValue([]);
    const client = mockApiClient({ getFeed });

    await listFeedHandler({ projectId: 'p1' }, client);

    expect(getFeed).toHaveBeenCalledWith('p1');
  });

  it('returns MCP content with JSON-stringified events', async () => {
    const events: FeedEvent[] = [{
      id: 'ev1', eventType: 'feature_created', actorName: 'bot',
      actorType: 'agent', description: 'Created feature', createdAt: '2026-01-01T00:00:00Z',
    }];
    const client = mockApiClient({ getFeed: vi.fn().mockResolvedValue(events) });

    const result = await listFeedHandler({ projectId: 'p1' }, client);

    expect(result.content).toHaveLength(1);
    expect(result.content[0]!.type).toBe('text');
    const parsed: unknown = JSON.parse(result.content[0]!.text);
    expect(parsed).toEqual(events);
  });

  it('returns error content when API call fails', async () => {
    const client = mockApiClient({
      getFeed: vi.fn().mockRejectedValue(new Error('500: Internal server error')),
    });

    const result = await listFeedHandler({ projectId: 'bad' }, client);

    expect(result.isError).toBe(true);
    expect(result.content[0]!.text).toContain('500');
  });
});
