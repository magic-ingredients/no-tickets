import { vi } from 'vitest';
import type { ApiClient } from '../../sdk/api-client.js';

export function mockApiClient(overrides: Partial<ApiClient> = {}): ApiClient {
  return {
    getBoard: vi.fn(),
    getFeed: vi.fn(),
    createEpic: vi.fn(),
    createFeature: vi.fn(),
    createFix: vi.fn(),
    updateFeature: vi.fn(),
    moveToPhase: vi.fn(),
    assignFeature: vi.fn(),
    breakDown: vi.fn(),
    ...overrides,
  };
}
