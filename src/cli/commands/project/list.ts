import { maskToken, readConfigSync, type ProjectEntry } from './config-io.js';

export async function runProjectList(): Promise<number> {
  const { config, exists } = readConfigSync();
  const projects = (exists ? config.projects ?? {} : {}) as Record<string, Partial<ProjectEntry>>;
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
