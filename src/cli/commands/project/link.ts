import {
  configPath,
  maskToken,
  readConfigSync,
  writeConfigSync,
  type ConfigShape,
  type ProjectEntry,
} from './config-io.js';

export interface ProjectLinkOptions {
  readonly name: string;
  readonly profile: string;
  readonly token: string;
  readonly force?: boolean;
}

export async function runProjectLink(options: ProjectLinkOptions): Promise<number> {
  if (options.name.length === 0) {
    console.error('project link: <name> is required');
    return 1;
  }
  if (options.profile.length === 0) {
    console.error('project link: --profile <name> is required');
    return 1;
  }
  if (options.token.length === 0) {
    console.error('project link: --token <nt_push_…> is required');
    return 1;
  }

  const { config, exists } = readConfigSync();
  if (!exists) {
    console.error(
      `project link: ${configPath()} does not exist. ` +
        `Define profiles first (or run \`nt init --profile ${options.profile}\` to create the file).`,
    );
    return 1;
  }

  // Verify the referenced profile exists. Without this, the user could
  // happily link a project that points at nothing — `nt publish` would
  // then fail with a confusing "profile not defined" at use time.
  const profiles = (config.profiles ?? {}) as Record<string, unknown>;
  if (!Object.hasOwn(profiles, options.profile)) {
    const available = Object.keys(profiles);
    const hint = available.length > 0 ? ` Available: ${available.join(', ')}.` : '';
    console.error(
      `project link: profile "${options.profile}" is not defined in ${configPath()}.${hint}`,
    );
    return 1;
  }

  const projects = (config.projects ?? {}) as Record<string, ProjectEntry>;
  if (Object.hasOwn(projects, options.name) && options.force !== true) {
    console.error(
      `project link: "${options.name}" is already linked. Re-run with --force to overwrite.`,
    );
    return 1;
  }

  const next: ConfigShape = {
    ...config,
    projects: {
      ...projects,
      [options.name]: { profile: options.profile, pushToken: options.token },
    },
  };
  writeConfigSync(next);

  console.log(
    `Linked project "${options.name}" → profile "${options.profile}" (token ${maskToken(options.token)}).`,
  );
  return 0;
}
