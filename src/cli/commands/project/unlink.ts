import {
  ConfigCorruptError,
  configPath,
  readConfigSync,
  writeConfigSync,
  type ConfigShape,
  type ProjectEntry,
} from './config-io.js';

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

export interface ProjectUnlinkOptions {
  readonly name: string;
}

export async function runProjectUnlink(options: ProjectUnlinkOptions): Promise<number> {
  if (options.name.length === 0) {
    console.error('project unlink: <name> is required');
    return 1;
  }

  let config: ConfigShape;
  let exists: boolean;
  try {
    ({ config, exists } = readConfigSync());
  } catch (err) {
    if (err instanceof ConfigCorruptError) {
      console.error(`project unlink: ${err.message}`);
      return 1;
    }
    throw err;
  }

  if (!exists) {
    console.error(`project unlink: ${configPath()} does not exist (no projects registered).`);
    return 1;
  }

  const projects = isRecord(config.projects) ? (config.projects as Record<string, ProjectEntry>) : {};
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
