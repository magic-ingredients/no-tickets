#!/usr/bin/env node
// Generate crates/nt-schemas/schemas/event-types.bundle.json by converting
// every Zod schema in @magic-ingredients/no-tickets-schemas to JSON Schema
// (Draft 2020-12) via Zod 4's built-in `toJSONSchema()`.
//
// Re-run this script when the schemas package is bumped. The generated
// file is committed so Rust callers (nt-cli / nt-mcp) can `include_str!`
// it without a runtime fetch.
//
// Known divergence: Zod `.refine()` predicates and other custom
// validators do NOT survive the conversion. The exported JSON Schema
// covers shape (type, required, format, length, enum) only. Server-side
// Zod validation still catches everything; local Rust validation is a
// strict subset of server validation, never a superset.

import { writeFileSync, mkdirSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { byTypeId } from '@magic-ingredients/no-tickets-schemas';

const __dirname = dirname(fileURLToPath(import.meta.url));

// The package doesn't export its own package.json subpath; read it
// from disk by resolving the package's main entry then walking up.
const schemasPkgVersion = JSON.parse(
  readFileSync(
    resolve(
      __dirname,
      '..',
      'node_modules',
      '@magic-ingredients',
      'no-tickets-schemas',
      'package.json',
    ),
    'utf8',
  ),
).version;

// IMPORTANT: use each schema's own `.toJSONSchema()` instance method.
// Importing `z.toJSONSchema` from a top-level `zod` install runs the
// conversion against schema instances from a different zod copy (pnpm
// resolves the schemas package's `zod` separately) and the cross-
// instance type check silently drops `.min()` / `.format()` /
// `.pattern()` constraints. Using the instance method keeps the
// conversion within the same zod copy that built the schema.

const bundle = {
  bundleVersion: schemasPkgVersion,
  generatedFrom: `@magic-ingredients/no-tickets-schemas@${schemasPkgVersion}`,
  jsonSchemaDraft: 'https://json-schema.org/draft/2020-12/schema',
  schemas: {},
};

const typeIds = Object.keys(byTypeId).sort();
for (const typeId of typeIds) {
  bundle.schemas[typeId] = byTypeId[typeId].toJSONSchema();
}

const outPath = resolve(
  __dirname,
  '..',
  'crates',
  'nt-schemas',
  'schemas',
  'event-types.bundle.json',
);
mkdirSync(dirname(outPath), { recursive: true });
writeFileSync(outPath, JSON.stringify(bundle, null, 2) + '\n');

console.log(
  `Wrote ${typeIds.length} schemas (${schemasPkgVersion}) → ${outPath}`,
);
