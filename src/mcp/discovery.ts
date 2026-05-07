// Public bundle for the MCP discovery surface. Consumers (the MCP server
// registration code, future create-server wiring, or any embedder that
// wants to drive the same handlers from a different transport) import from
// this module rather than reaching into individual files.

export { listEventTypesTool } from './tools/list-event-types.js';
export { describeEventTypeTool } from './tools/describe-event-type.js';
export { publishEventTool } from './tools/publish-event.js';
export { runInteractionTool } from './tools/run-interaction.js';
export { createSubjectTool } from './tools/create-subject.js';
export type { ToolDescriptor } from './tools/tool-descriptor.js';

export {
  handleListEventTypes,
  handleDescribeEventType,
  handlePublishEvent,
  handleRunInteraction,
  handleCreateSubject,
  type ToolHandlerDeps,
  type ListEventTypesArgs,
  type ListEventTypesResult,
  type DescribeEventTypeArgs,
  type DescribeEventTypeResult,
  type PublishEventArgs,
  type PublishEventResult,
  type RunInteractionArgs,
  type RunInteractionResult,
  type CreateSubjectArgs,
  type CreateSubjectResult,
} from './tools/handlers.js';

export { sourceFromTransport, type TransportHints } from './lib/source-from-transport.js';
export {
  mapErrorToToolResult,
  type StructuredToolError,
  type StructuredToolErrorCode,
  type StructuredToolFailure,
} from './lib/error-mapping.js';
