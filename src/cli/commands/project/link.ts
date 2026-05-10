// Stub — implementation lands in the GREEN commit.

export interface ProjectLinkOptions {
  readonly name: string;
  readonly profile: string;
  readonly token: string;
  readonly force?: boolean;
}

export async function runProjectLink(_options: ProjectLinkOptions): Promise<number> {
  throw new Error('runProjectLink: not implemented');
}
