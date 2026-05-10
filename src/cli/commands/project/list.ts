import { ConfigCorruptError, maskToken, readConfigSync, type ProjectEntry } from './config-io.js';

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

export async function runProjectList(): Promise<number> {
  let config: ReturnType<typeof readConfigSync>['config'];
  let exists: boolean;
  try {
    ({ config, exists } = readConfigSync());
  } catch (err) {
    if (err instanceof ConfigCorruptError) {
      console.error(`project list: ${err.message}`);
      return 1;
    }
    throw err;
  }

  // Defensive narrow — a malformed `projects: 42` would survive a blanket
  // cast and crash on Object.keys. isRecord collapses any non-object to
  // "no projects registered" rather than throwing.
  const rawProjects = exists ? config.projects : undefined;
  const projects = isRecord(rawProjects) ? (rawProjects as Record<string, Partial<ProjectEntry>>) : {};
  const names = Object.keys(projects).sort();

  if (names.length === 0) {
    console.log('no projects registered.');
    return 0;
  }

  for (const name of names) {
    const entry = projects[name] ?? {};
    const profile = typeof entry.profile === 'string' ? entry.profile : '<missing>';
    const token = typeof entry.pushToken === 'string' ? maskToken(entry.pushToken) : '<missing>';
    console.log(`${name}\t${profile}\t${token}`);
  }
  return 0;
}
