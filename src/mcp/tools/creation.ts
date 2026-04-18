import type { ApiClient } from '../../sdk/api-client.js';
import { toolSuccess, toolError, type ToolResult } from './types.js';

interface CreateEpicInput {
  readonly projectId: string;
  readonly title: string;
  readonly description?: string;
}

interface CreateFeatureInput {
  readonly projectId: string;
  readonly epicId: string;
  readonly title: string;
  readonly description?: string;
}

interface CreateFixInput {
  readonly projectId: string;
  readonly epicId: string;
  readonly title: string;
  readonly description?: string;
}

export async function createEpicHandler(
  params: CreateEpicInput,
  client: ApiClient,
): Promise<ToolResult> {
  try {
    const result = await client.createEpic(params);
    return toolSuccess(result);
  } catch (err) {
    return toolError(err);
  }
}

export async function createFeatureHandler(
  params: CreateFeatureInput,
  client: ApiClient,
): Promise<ToolResult> {
  try {
    const result = await client.createFeature(params);
    return toolSuccess(result);
  } catch (err) {
    return toolError(err);
  }
}

export async function createFixHandler(
  params: CreateFixInput,
  client: ApiClient,
): Promise<ToolResult> {
  try {
    const result = await client.createFix(params);
    return toolSuccess(result);
  } catch (err) {
    return toolError(err);
  }
}
