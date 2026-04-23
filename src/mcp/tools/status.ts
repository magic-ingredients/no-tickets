import { describeAuthStatus, NOT_AUTHENTICATED_MESSAGE } from '../../sdk/auth.js';
import { toolSuccess, toolError, type ToolResult } from './types.js';

export function handleStatus(): ToolResult {
  try {
    return toolSuccess(describeAuthStatus());
  } catch {
    return toolError(new Error(NOT_AUTHENTICATED_MESSAGE));
  }
}
