// =============================================================================
// Core domain types — the canonical definitions for the no-tickets format.
// Pure type definitions, zero runtime dependencies.
//
// Imported by: notickets-mcp, notickets-service, tiny-brain-core, and any
// third-party integration via '@magic-ingredients/no-tickets-client/types'.
// =============================================================================

// -- Enums as unions ----------------------------------------------------------

export type Phase = 'ideation' | 'development' | 'testing' | 'review' | 'done';

export type DocumentType = 'epic' | 'feature' | 'fix';

export type TaskStatus = 'not_started' | 'in_progress' | 'completed';

export type EntityStatus = 'not_started' | 'in_progress' | 'completed';

export type AssigneeType = 'human' | 'agent';

// -- Frontmatter (what lives in the YAML of each .md file) --------------------

export interface EpicFrontmatter {
  readonly id: string;
  readonly type: 'epic';
  readonly title: string;
  readonly status: EntityStatus;
  readonly created: string;
  readonly updated: string;
  readonly meta?: Readonly<Record<string, unknown>>;
}

export interface FeatureFrontmatter {
  readonly id: string;
  readonly type: 'feature' | 'fix';
  readonly epic: string;
  readonly title: string;
  readonly phase: Phase;
  readonly status: EntityStatus;
  readonly assignee?: string;
  readonly assignee_type?: AssigneeType;
  readonly created: string;
  readonly updated: string;
  readonly meta?: Readonly<Record<string, unknown>>;
}

// -- Parsed documents (frontmatter + extracted content) -----------------------

export interface ParsedEpic {
  readonly frontmatter: EpicFrontmatter;
  readonly description: string;
  readonly goals: readonly string[];
  readonly filePath: string;
}

export interface ParsedFeature {
  readonly frontmatter: FeatureFrontmatter;
  readonly description: string;
  readonly tasks: readonly ParsedTask[];
  readonly acceptanceCriteria: readonly string[];
  readonly filePath: string;
}

export interface ParsedTask {
  readonly number: number;
  readonly title: string;
  readonly status: TaskStatus;
}

export interface ParseResult {
  readonly epics: readonly ParsedEpic[];
  readonly features: readonly ParsedFeature[];
}

// -- State (computed from parsed documents, used for sync) --------------------

export interface TaskSummary {
  readonly total: number;
  readonly completed: number;
}

export interface TestSummary {
  readonly total: number;
  readonly passing: number;
}

export interface FeatureState {
  readonly id: string;
  readonly epicId: string;
  readonly title: string;
  readonly type: 'feature' | 'fix';
  readonly phase: Phase;
  readonly assignee?: string;
  readonly assigneeType?: AssigneeType;
  readonly tasks: TaskSummary;
  readonly tests: TestSummary;
  readonly meta?: Readonly<Record<string, unknown>>;
}

export interface EpicState {
  readonly id: string;
  readonly title: string;
  readonly status: EntityStatus;
  readonly features: readonly FeatureState[];
}

export interface StateSnapshot {
  readonly version: number;
  readonly epics: readonly EpicState[];
  readonly computedAt: string;
}

// -- Sync (delta between snapshots) -------------------------------------------

export interface FeatureUpdate {
  readonly id: string;
  readonly changes: Readonly<Record<string, { readonly from: unknown; readonly to: unknown }>>;
}

export interface SyncDelta {
  readonly added: readonly FeatureState[];
  readonly updated: readonly FeatureUpdate[];
  readonly removed: readonly string[];
  readonly isEmpty: boolean;
}

// -- Validation ---------------------------------------------------------------

export interface ValidationError {
  readonly file: string;
  readonly line?: number;
  readonly field?: string;
  readonly message: string;
  readonly suggestion?: string;
}

export interface ValidationResult {
  readonly valid: boolean;
  readonly errors: readonly ValidationError[];
}

// -- File entry (input to parser) ---------------------------------------------

export interface FileEntry {
  readonly path: string;
  readonly content: string;
}

// -- Config -------------------------------------------------------------------

export interface SyncConfig {
  readonly teamId: string;
  readonly projectId: string;
  readonly token: string;
  readonly apiUrl: string;
}

export interface NoTicketsConfig {
  readonly apiUrl: string;
  readonly token: string;
  readonly teamId?: string;
  readonly projectId?: string;
}

// -- Wire-format envelope types (re-exported for @magic-ingredients/no-tickets/types subpath) --

export type { Source } from './source.js';
export type { Event } from './event.js';
export type { Subject, SubjectRef } from './subject.js';
export type {
  InteractionRequest,
  InteractionResponse,
  InteractionEventRef,
} from './interaction.js';
export type { TypeIdParts } from './type-id.js';

