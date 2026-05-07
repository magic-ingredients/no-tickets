import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';
import { z } from 'zod';

// Path stable across src/core/source.ts (vitest) and dist/core/source.js
// (shipped npm tarball) — package.json sits two levels above in both layouts.
function readSdkVersion(): string {
  const here = dirname(fileURLToPath(import.meta.url));
  const pkgPath = resolve(here, '..', '..', 'package.json');
  const pkg = JSON.parse(readFileSync(pkgPath, 'utf-8')) as { version: string };
  return pkg.version;
}

export const SDK_VERSION: string = readSdkVersion();

const attributeValueSchema = z.union([z.string(), z.number(), z.boolean()]);

export const sourceSchema = z
  .object({
    name: z.string().min(1),
    sdkVersion: z.string().min(1),
    version: z.string().optional(),
    attributes: z.record(z.string(), attributeValueSchema).optional(),
  })
  .strict();

export type Source = z.infer<typeof sourceSchema>;

export type SourceOverride = Partial<Source>;

export function mergeSource(auto: Source, override?: SourceOverride): Source {
  if (!override) return auto;

  const merged: Source = {
    name: override.name ?? auto.name,
    sdkVersion: override.sdkVersion ?? auto.sdkVersion,
  };

  const version = override.version ?? auto.version;
  if (version !== undefined) merged.version = version;

  const attributes = mergeAttributes(auto.attributes, override.attributes);
  if (attributes !== undefined) merged.attributes = attributes;

  return merged;
}

function mergeAttributes(
  auto: Record<string, string | number | boolean> | undefined,
  override: Record<string, string | number | boolean> | undefined,
): Record<string, string | number | boolean> | undefined {
  if (auto === undefined && override === undefined) return undefined;
  return { ...auto, ...override };
}
