import {
  configPath,
  readConfigSync,
  writeConfigSync,
  type ConfigShape,
  type ProjectEntry,
} from './config-io.js';

export interface ProjectUnlinkOptions {
  readonly name: string;
}

export async function runProjectUnlink(options: ProjectUnlinkOptions): Promise<number> {
  if (options.name.length === 0) {
    console.error('project unlink: <name> is required');
    return 1;
  }

  const { config, exists } = readConfigSync();
  if (!exists) {
    console.error(`project unlink: ${configPath()} does not exist (no projects registered).`);
    return 1;
  }

  const projects = (config.projects ?? {}) as Record<string, ProjectEntry>;
  if (!Object.hasOwn(projects, options.name)) {
    console.error(`project unlink: "${options.name}" is not registered.`);
    return 1;
  }

  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const { [options.name]: _removed, ...rest } = projects;
  const next: ConfigShape = { ...config, projects: rest };
  writeConfigSync(next);

  console.log(`Unlinked project "${options.name}".`);
  return 0;
}
