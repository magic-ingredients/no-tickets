export interface ToolResult {
  readonly content: Array<{ readonly type: 'text'; readonly text: string }>;
  readonly isError?: boolean;
  readonly [key: string]: unknown;
}

export function toolSuccess(data: unknown): ToolResult {
  return {
    content: [{ type: 'text', text: JSON.stringify(data) }],
  };
}

export function toolError(err: unknown): ToolResult {
  return {
    isError: true,
    content: [{ type: 'text', text: err instanceof Error ? err.message : String(err) }],
  };
}
