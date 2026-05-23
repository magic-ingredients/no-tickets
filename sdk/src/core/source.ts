import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';
import { z } from 'zod';

const packageJsonSchema = z.object({ version: z.string().min(1) });

// Path stable across src/core/source.ts (vitest) and dist/core/source.js
// (shipped npm tarball) — package.json sits two levels above in both layouts.
function readSdkVersion(): string {
  const here = dirname(fileURLToPath(import.meta.url));
  const pkgPath = resolve(here, '..', '..', 'package.json');
  const raw: unknown = JSON.parse(readFileSync(pkgPath, 'utf-8'));
  return packageJsonSchema.parse(raw).version;
}

export const SDK_VERSION: string = readSdkVersion();

const attributeValueSchema = z.union([z.string(), z.number(), z.boolean()]);

export const sourceSchema = z.object({
  name: z.string().min(1),
  sdkVersion: z.string().min(1),
  version: z.string().optional(),
  attributes: z.record(z.string(), attributeValueSchema).optional(),
});

export type Source = z.infer<typeof sourceSchema>;

// Empty strings are treated as gaps so the merged result stays conformant
// (sourceSchema enforces .min(1) on name/sdkVersion).
export function mergeSource(auto: Source, override?: Partial<Source>): Source {
  const merged: Source = {
    name: nonEmpty(override?.name) ?? auto.name,
    sdkVersion: nonEmpty(override?.sdkVersion) ?? auto.sdkVersion,
  };

  const version = nonEmpty(override?.version) ?? auto.version;
  if (version !== undefined) merged.version = version;

  const attributes = mergeAttributes(auto.attributes, override?.attributes);
  if (attributes !== undefined) merged.attributes = attributes;

  return merged;
}

function nonEmpty(value: string | undefined): string | undefined {
  return value !== undefined && value.length > 0 ? value : undefined;
}

function mergeAttributes(
  auto: Record<string, string | number | boolean> | undefined,
  override: Record<string, string | number | boolean> | undefined,
): Record<string, string | number | boolean> | undefined {
  if (auto === undefined && override === undefined) return undefined;
  return { ...auto, ...override };
}
