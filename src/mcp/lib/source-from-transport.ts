import type { Source } from '../../core/source.js';
import { SDK_VERSION } from '../../core/source.js';

export interface TransportHints {
  readonly client?: string;
  readonly clientVersion?: string;
}

/** Build the MCP server-side Source from transport-supplied hints.
 *  Defaults to `{ name: 'mcp', attributes: { client: 'unknown' } }` when no
 *  hints are available; clientVersion is only attached when client is also
 *  present (no orphaned version metadata). */
export function sourceFromTransport(hints: TransportHints): Source {
  const client =
    hints.client !== undefined && hints.client.length > 0 ? hints.client : 'unknown';
  const attributes: Record<string, string> = { client };
  if (client !== 'unknown' && hints.clientVersion !== undefined && hints.clientVersion.length > 0) {
    attributes['clientVersion'] = hints.clientVersion;
  }
  return {
    name: 'mcp',
    sdkVersion: SDK_VERSION,
    attributes,
  };
}
