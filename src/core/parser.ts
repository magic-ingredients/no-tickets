import matter from 'gray-matter';
import type {
  FileEntry,
  ParsedEpic,
  ParsedFeature,
  ParsedTask,
  ParseResult,
  TaskStatus,
} from './types.js';

// -- Frontmatter parsing ------------------------------------------------------

/**
 * Parse YAML frontmatter from markdown content using gray-matter.
 * Returns the frontmatter object and the remaining body content.
 * Pure function — no I/O.
 */
export function parseFrontmatter(content: string): { data: Record<string, unknown>; body: string } {
  const { data, content: body } = matter(content);

  // gray-matter auto-converts date-like strings to Date objects.
  // Coerce them back to YYYY-MM-DD strings for consistency.
  const normalized: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(data)) {
    if (value instanceof Date) {
      normalized[key] = value.toISOString().split('T')[0];
    } else {
      normalized[key] = value;
    }
  }

  return { data: normalized, body };
}

// -- Task parsing -------------------------------------------------------------

const TASK_HEADING_REGEX = /^###\s+(\d+)\.\s+(.+)$/;
const TASK_STATUS_REGEX = /^status:\s*(\S+)/;

const VALID_TASK_STATUSES: ReadonlySet<TaskStatus> = new Set<TaskStatus>([
  'not_started',
  'in_progress',
  'completed',
]);

function isTaskStatus(value: string): value is TaskStatus {
  return VALID_TASK_STATUSES.has(value as TaskStatus);
}

/**
 * Extract tasks from a markdown body's ## Tasks section.
 * Pure function — operates on string content only.
 */
export function parseTasks(body: string): readonly ParsedTask[] {
  const tasksSection = extractSection(body, 'Tasks');
  if (!tasksSection) return [];

  const tasks: ParsedTask[] = [];
  const lines = tasksSection.split('\n');

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (!line) continue;

    const headingMatch = TASK_HEADING_REGEX.exec(line);
    if (!headingMatch?.[1] || !headingMatch[2]) continue;

    const number = parseInt(headingMatch[1], 10);
    const title = headingMatch[2].trim();

    let status: TaskStatus = 'not_started';
    for (let j = i + 1; j < lines.length; j++) {
      const nextLine = lines[j]?.trim();
      if (!nextLine) continue;

      const statusMatch = TASK_STATUS_REGEX.exec(nextLine);
      if (statusMatch?.[1] && isTaskStatus(statusMatch[1])) {
        status = statusMatch[1];
      }
      break;
    }

    tasks.push({ number, title, status });
  }

  return tasks;
}

// -- Section extraction -------------------------------------------------------

function escapeRegex(str: string): string {
  return str.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/**
 * Extract content under a ## heading until the next ## heading or end of document.
 * Pure function.
 */
export function extractSection(body: string, sectionName: string): string | undefined {
  const escaped = escapeRegex(sectionName);
  const regex = new RegExp(`^##\\s+${escaped}\\s*$`, 'm');
  const match = regex.exec(body);
  if (!match) return undefined;

  const start = match.index + match[0].length;
  const nextSection = body.indexOf('\n## ', start);
  const end = nextSection === -1 ? body.length : nextSection;

  return body.slice(start, end).trim();
}

// -- Goal extraction ----------------------------------------------------------

/**
 * Extract goals from an epic's body (bulleted list under ## Goals).
 * Pure function.
 */
export function parseGoals(body: string): readonly string[] {
  const section = extractSection(body, 'Goals');
  if (!section) return [];

  return section
    .split('\n')
    .map((line) => line.replace(/^[-*]\s*/, '').trim())
    .filter((line) => line.length > 0);
}

// -- Acceptance criteria extraction -------------------------------------------

/**
 * Extract acceptance criteria from a feature body (checkbox list under ## Acceptance Criteria).
 * Pure function.
 */
export function parseAcceptanceCriteria(body: string): readonly string[] {
  const section = extractSection(body, 'Acceptance Criteria');
  if (!section) return [];

  return section
    .split('\n')
    .map((line) => line.replace(/^-\s*(\[[ x]\]\s*)?/, '').trim())
    .filter((line) => line.length > 0);
}

// -- Description extraction ---------------------------------------------------

/**
 * Extract description text (content between the # heading and first ## heading).
 * Pure function.
 */
export function parseDescription(body: string): string {
  const firstH1End = body.indexOf('\n## ');
  const content = firstH1End === -1 ? body : body.slice(0, firstH1End);

  return content
    .replace(/^#\s+.*\n/, '')
    .trim();
}

// -- Meta extraction ----------------------------------------------------------

function parseMeta(data: Record<string, unknown>): Readonly<Record<string, unknown>> | undefined {
  const meta = data['meta'];
  if (typeof meta !== 'object' || meta === null) return undefined;
  return meta as Readonly<Record<string, unknown>>;
}

// -- Document assembly --------------------------------------------------------

/**
 * Assemble a ParsedEpic from frontmatter data and body content.
 * Pure function.
 */
export function assembleEpic(
  data: Record<string, unknown>,
  body: string,
  filePath: string,
): ParsedEpic {
  return {
    frontmatter: {
      id: String(data['id'] ?? ''),
      type: 'epic',
      title: String(data['title'] ?? ''),
      status: normalizeStatus(data['status']),
      created: String(data['created'] ?? ''),
      updated: String(data['updated'] ?? ''),
      meta: parseMeta(data),
    },
    description: parseDescription(body),
    goals: parseGoals(body),
    filePath,
  };
}

/**
 * Assemble a ParsedFeature from frontmatter data and body content.
 * Pure function.
 */
export function assembleFeature(
  data: Record<string, unknown>,
  body: string,
  filePath: string,
): ParsedFeature {
  const docType = String(data['type'] ?? 'feature');

  return {
    frontmatter: {
      id: String(data['id'] ?? ''),
      type: docType === 'fix' ? 'fix' : 'feature',
      epic: String(data['epic'] ?? ''),
      title: String(data['title'] ?? ''),
      phase: normalizePhase(data['phase']),
      status: normalizeStatus(data['status']),
      assignee: data['assignee'] ? String(data['assignee']) : undefined,
      assignee_type: normalizeAssigneeType(data['assignee_type']),
      created: String(data['created'] ?? ''),
      updated: String(data['updated'] ?? ''),
      meta: parseMeta(data),
    },
    description: parseDescription(body),
    tasks: parseTasks(body),
    acceptanceCriteria: parseAcceptanceCriteria(body),
    filePath,
  };
}

// -- Parse a set of files -----------------------------------------------------

/**
 * Parse a collection of markdown file entries into structured data.
 * Pure function — accepts file contents as input, no filesystem access.
 */
export function parseFiles(files: readonly FileEntry[]): ParseResult {
  const epics: ParsedEpic[] = [];
  const features: ParsedFeature[] = [];

  for (const file of files) {
    const { data, body } = parseFrontmatter(file.content);
    const docType = String(data['type'] ?? '');

    if (docType === 'epic') {
      epics.push(assembleEpic(data, body, file.path));
    } else if (docType === 'feature' || docType === 'fix') {
      features.push(assembleFeature(data, body, file.path));
    }
  }

  return { epics, features };
}

// -- Normalization helpers ----------------------------------------------------

function normalizeStatus(value: unknown): 'not_started' | 'in_progress' | 'completed' {
  const s = String(value ?? 'not_started');
  if (s === 'in_progress' || s === 'completed') return s;
  return 'not_started';
}

function normalizePhase(value: unknown): 'ideation' | 'development' | 'testing' | 'review' | 'done' {
  const s = String(value ?? 'ideation');
  if (s === 'development' || s === 'testing' || s === 'review' || s === 'done') return s;
  return 'ideation';
}

function normalizeAssigneeType(value: unknown): 'human' | 'agent' | undefined {
  const s = String(value ?? '');
  if (s === 'human' || s === 'agent') return s;
  return undefined;
}
