import type { ApiClient, CreateEpicParams, CreateFeatureParams, CreateFixParams } from '../../sdk/api-client.js';
import { toolSuccess, toolError, type ToolResult } from './types.js';

export async function createEpicHandler(
  params: CreateEpicParams,
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
  params: CreateFeatureParams,
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
  params: CreateFixParams,
  client: ApiClient,
): Promise<ToolResult> {
  try {
    const result = await client.createFix(params);
    return toolSuccess(result);
  } catch (err) {
    return toolError(err);
  }
}
