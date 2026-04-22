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

export interface SessionState {
  readonly agent: string;
  readonly agentType: AssigneeType;
  readonly active: boolean;
  readonly since: string;
}

export interface StateSnapshot {
  readonly version: number;
  readonly epics: readonly EpicState[];
  readonly session?: SessionState;
  readonly pushedAt: string;
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

export interface PushResult {
  readonly success: boolean;
  readonly changesApplied: number;
  readonly eventsGenerated: number;
}

// -- Board (what the dashboard renders) ---------------------------------------

export interface BoardColumn {
  readonly phase: Phase;
  readonly features: readonly FeatureState[];
}

export interface BoardState {
  readonly projectId: string;
  readonly columns: readonly BoardColumn[];
}

// -- Feed (activity events) ---------------------------------------------------

export interface FeedEvent {
  readonly id: string;
  readonly eventType: string;
  readonly actorName: string;
  readonly actorType: AssigneeType;
  readonly description: string;
  readonly featureId?: string;
  readonly createdAt: string;
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

// =============================================================================
// Push Payload v2 — generic core envelope with published extension schemas.
// =============================================================================

// -- Enums as unions (v2) -----------------------------------------------------

export type WorkEntityType = 'epic' | 'feature' | 'task';

export type EngineeringPhase = 'red' | 'green' | 'refactor' | 'review' | 'complete';

export type AcceptanceStatus = 'unreviewed' | 'accepted' | 'changes_requested';

export type Priority = 'critical' | 'high' | 'medium' | 'low';

export type CodeQualitySource = 'local' | 'ci';

// -- Core envelope (Task 1) ---------------------------------------------------

export interface Push {
  readonly projectId: string;
  readonly timestamp: string;
  readonly session?: Session;

  readonly work?: WorkSchema;
  readonly engineering?: EngineeringSchema;
  readonly product?: ProductSchema;
  readonly codeQuality?: CodeQualitySchema;

  readonly custom?: Readonly<Record<string, unknown>>;
}

export interface Session {
  readonly agent: string;
  readonly agentType: AssigneeType;
  readonly model?: string;
  readonly vendor?: string;
  readonly environment?: PushEnvironment;
  readonly duration?: number;
  readonly result?: string;
  readonly meta?: Readonly<Record<string, unknown>>;
}

export interface PushEnvironment {
  readonly os?: string;
  readonly runtime?: string;
  readonly ci?: boolean;
  readonly ciProvider?: string;
}

// -- Schema: "work" -----------------------------------------------------------

export interface WorkSchema {
  readonly entities: readonly WorkEntity[];
}

export interface WorkEntity {
  readonly id: string;
  readonly type: WorkEntityType;
  readonly parentId?: string;
  readonly title: string;
  readonly status: EntityStatus;
  readonly assignee?: string;
  readonly assigneeType?: AssigneeType;
  readonly meta?: Readonly<Record<string, unknown>>;
}

// -- Schema: "engineering" ----------------------------------------------------

export interface EngineeringSchema {
  readonly tasks?: readonly EngineeringTask[];
  readonly meta?: Readonly<Record<string, unknown>>;
}

export interface EngineeringTask {
  readonly entityId: string;
  readonly phase?: EngineeringPhase;
  readonly commitSha?: string;
  readonly startedAt?: string;
  readonly completedAt?: string;
  readonly duration?: number;
  readonly reviews?: readonly EngineeringReview[];
  readonly meta?: Readonly<Record<string, unknown>>;
}

export interface EngineeringReview {
  readonly reviewer: string;
  readonly verdict: string;
  readonly findings?: number;
}

// -- Schema: "product" --------------------------------------------------------

export interface ProductSchema {
  readonly updates: readonly ProductUpdate[];
  readonly meta?: Readonly<Record<string, unknown>>;
}

export interface ProductUpdate {
  readonly entityId: string;
  readonly acceptance?: AcceptanceStatus;
  readonly priority?: Priority;
  readonly labels?: readonly string[];
  readonly releaseId?: string;
  readonly notes?: string;
  readonly meta?: Readonly<Record<string, unknown>>;
}

// -- Schema: "codeQuality" ----------------------------------------------------

export interface CodeQualitySchema {
  readonly score: number;
  readonly grade?: string;
  readonly source?: CodeQualitySource;
  readonly entityId?: string;
  readonly categories?: Readonly<Record<string, number>>;
  readonly meta?: Readonly<Record<string, unknown>>;
}
