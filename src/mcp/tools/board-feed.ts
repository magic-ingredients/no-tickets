import type { ApiClient } from '../../sdk/api-client.js';
import { toolSuccess, toolError, type ToolResult } from './types.js';

export async function listBoardHandler(
  params: { readonly projectId: string },
  client: ApiClient,
): Promise<ToolResult> {
  try {
    const board = await client.getBoard(params.projectId);
    return toolSuccess(board);
  } catch (err) {
    return toolError(err);
  }
}

export async function listFeedHandler(
  params: { readonly projectId: string },
  client: ApiClient,
): Promise<ToolResult> {
  try {
    const events = await client.getFeed(params.projectId);
    return toolSuccess(events);
  } catch (err) {
    return toolError(err);
  }
}
