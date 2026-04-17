import type { ApiClient } from '../../sdk/api-client.js';

interface ToolResult {
  readonly content: Array<{ readonly type: 'text'; readonly text: string }>;
  readonly isError?: boolean;
}

export async function listBoardHandler(
  params: { readonly projectId: string },
  client: ApiClient,
): Promise<ToolResult> {
  try {
    const board = await client.getBoard(params.projectId);
    return {
      content: [{ type: 'text', text: JSON.stringify(board) }],
    };
  } catch (err) {
    return {
      isError: true,
      content: [{ type: 'text', text: err instanceof Error ? err.message : String(err) }],
    };
  }
}

export async function listFeedHandler(
  params: { readonly projectId: string },
  client: ApiClient,
): Promise<ToolResult> {
  try {
    const events = await client.getFeed(params.projectId);
    return {
      content: [{ type: 'text', text: JSON.stringify(events) }],
    };
  } catch (err) {
    return {
      isError: true,
      content: [{ type: 'text', text: err instanceof Error ? err.message : String(err) }],
    };
  }
}
