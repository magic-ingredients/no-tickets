/**
 * Smoke test for `publish()` against a real server.
 *
 * Auth/URL resolution — mutually exclusive shapes:
 *   1. --project <name>
 *        Reads token + URL from ~/.notickets/config.json via resolveProjectAuth.
 *        Use for local dev once `nt project link` has registered the project.
 *
 *   2. --token-env-var <NAME> [--url <url>]
 *        Reads token from the env var named <NAME>. --url overrides the API
 *        URL (otherwise NO_TICKETS_API_URL env / production defaults).
 *        Use for CI multi-project — caller names its own env var per project.
 *
 *   3. --profile <name> [+ NO_TICKETS_TOKEN env]
 *        Legacy single-project path: --profile picks URL pair from config.json;
 *        NO_TICKETS_TOKEN supplies the token. Unchanged.
 *
 *   4. Defaults (no flags)
 *        NO_TICKETS_TOKEN env + NO_TICKETS_API_URL env / production URLs.
 *
 * Each typed error class prints something useful. A success prints the
 * server response so you can grep the dashboard for the event id.
 */

import { ZodError } from 'zod';
import { Client } from '../src/transport/client.js';
import { publish, type PublishEvent } from '../src/transport/events.js';
import {
  UnknownEventTypeError,
  EventValidationError,
  PermissionDeniedError,
  ServerError,
  HttpError,
  TransportError,
} from '../src/transport/errors.js';
import { resolveUrls } from '../src/sdk/url-resolver.js';
import { resolveAuth } from '../src/sdk/auth.js';
import { resolveProjectAuth } from '../src/sdk/projects.js';

function die(msg: string): never {
  console.error(msg);
  process.exit(1);
}

interface ParsedArgs {
  readonly project: string | undefined;
  readonly tokenEnvVar: string | undefined;
  readonly url: string | undefined;
  readonly profile: string | undefined;
  readonly positionals: readonly string[];
}

/** Tiny ad-hoc parser. Kept inline — the smoke script doesn't need the full
 *  CLI parser, just enough to thread the four auth/URL shapes documented
 *  above. Unknown flags fall into `positionals` so a misspelled flag
 *  surfaces loudly as `<type-id>` taking the wrong value. */
function parseArgs(argv: readonly string[]): ParsedArgs {
  const positionals: string[] = [];
  let project: string | undefined;
  let tokenEnvVar: string | undefined;
  let url: string | undefined;
  let profile: string | undefined;
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--project') {
      project = argv[i + 1];
      if (project === undefined) die('--project requires a value');
      i++;
      continue;
    }
    if (arg === '--token-env-var') {
      tokenEnvVar = argv[i + 1];
      if (tokenEnvVar === undefined) die('--token-env-var requires a value');
      i++;
      continue;
    }
    if (arg === '--url') {
      url = argv[i + 1];
      if (url === undefined) die('--url requires a value');
      i++;
      continue;
    }
    if (arg === '--profile') {
      profile = argv[i + 1];
      if (profile === undefined) die('--profile requires a value');
      i++;
      continue;
    }
    positionals.push(arg as string);
  }
  return { project, tokenEnvVar, url, profile, positionals };
}

const args = parseArgs(process.argv.slice(2));
const [typeId, rawData] = args.positionals;
if (typeId === undefined) {
  die(
    'usage: tsx scripts/smoke-publish.ts ' +
      '[--project <name> | --token-env-var <NAME> [--url <url>] | --profile <name>] ' +
      '<type-id> [data-json]',
  );
}

// Mutual exclusion — picking more than one auth source is almost always
// a mistake (mixing `--project myapp` with `--profile staging` resolves
// to whichever the script checks first, invisible from the invocation
// site). Surface the conflict early.
const sources = [
  args.project !== undefined ? '--project' : null,
  args.tokenEnvVar !== undefined ? '--token-env-var' : null,
  args.profile !== undefined ? '--profile' : null,
].filter((s): s is string => s !== null);
if (sources.length > 1) {
  die(`auth sources are mutually exclusive; got ${sources.join(' + ')}`);
}

let token: string;
let tokenSource: string;
let baseUrl: string;
let urlSource: string;

if (args.project !== undefined) {
  try {
    const auth = resolveProjectAuth(args.project);
    token = auth.token;
    tokenSource = `project:${args.project}`;
    baseUrl = auth.apiUrl;
    urlSource = `project:${args.project}`;
  } catch (err) {
    die(err instanceof Error ? err.message : String(err));
  }
} else if (args.tokenEnvVar !== undefined) {
  const envValue = process.env[args.tokenEnvVar];
  if (envValue === undefined || envValue.length === 0) {
    die(`--token-env-var ${args.tokenEnvVar}: env var is unset or empty`);
  }
  token = envValue;
  tokenSource = `env:${args.tokenEnvVar}`;
  if (args.url !== undefined) {
    baseUrl = args.url;
    urlSource = '--url';
  } else {
    try {
      const resolved = resolveUrls({});
      baseUrl = resolved.apiUrl;
      urlSource = resolved.source;
    } catch (err) {
      die(err instanceof Error ? err.message : String(err));
    }
  }
} else {
  try {
    const auth = resolveAuth();
    token = auth.token;
    tokenSource = `${auth.source} (${auth.tokenType})`;
  } catch (err) {
    die(err instanceof Error ? err.message : String(err));
  }
  try {
    const resolved = resolveUrls({
      ...(args.profile !== undefined && { profile: args.profile }),
    });
    baseUrl = args.url ?? resolved.apiUrl;
    urlSource = args.url !== undefined ? '--url' : resolved.source;
  } catch (err) {
    die(err instanceof Error ? err.message : String(err));
  }
}

let data: unknown = {};
if (rawData !== undefined) {
  try {
    data = JSON.parse(rawData);
  } catch (err) {
    die(`data is not valid JSON: ${err instanceof Error ? err.message : String(err)}`);
  }
}

const client = new Client({
  baseUrl,
  token,
  // Pin source so CI-detected env vars don't muddy attribution on a smoke.
  source: { name: 'sdk', sdkVersion: 'smoke-test' },
});

const event: PublishEvent = {
  type: typeId,
  data,
  dedupeKey: `smoke-${Date.now()}`,
};

console.error(`POST ${baseUrl}/v1/events  (urls: ${urlSource}, auth: ${tokenSource})`);
console.error(`type:      ${typeId}`);
console.error(`data:      ${JSON.stringify(data)}`);
console.error(`dedupeKey: ${event.dedupeKey}`);
console.error('');

try {
  const result = await publish(client, [event]);
  console.error('✅ published');
  console.log(JSON.stringify(result, null, 2));
  if (result.deduped > 0) {
    console.error('(deduped — server treated this dedupeKey as already seen)');
  }
} catch (err) {
  if (err instanceof UnknownEventTypeError) {
    console.error(`❌ unknown_event_type: ${err.typeId} (server batchIndex ${err.batchIndex})`);
    console.error('   → type id is not registered, or your token cannot write to it.');
  } else if (err instanceof EventValidationError) {
    console.error(`❌ event_validation for ${err.typeId} (batchIndex ${err.batchIndex}):`);
    for (const issue of err.issues) {
      console.error(`   - ${issue.path.join('.')}: ${issue.message}`);
    }
    console.error('   → wire path + type lookup OK; `data` shape is wrong.');
  } else if (err instanceof PermissionDeniedError) {
    console.error(`❌ permission_denied for domain "${err.domain}"`);
    console.error('   → auth resolved; token cannot write to this domain.');
  } else if (err instanceof ServerError) {
    console.error(`❌ server_error ${err.status}`);
    console.error(`   body: ${JSON.stringify(err.body, null, 2)}`);
  } else if (err instanceof HttpError) {
    console.error(`❌ http_error ${err.status}`);
    console.error(`   body: ${JSON.stringify(err.body, null, 2)}`);
  } else if (err instanceof ZodError) {
    console.error('❌ response shape rejected by client schema:');
    for (const issue of err.issues) {
      console.error(`   - ${issue.path.join('.')}: ${issue.message}`);
    }
    console.error('   → server response does NOT match { ingested, deduped, ids } — server/client drift.');
  } else if (err instanceof TransportError) {
    console.error(`❌ transport error: ${err.message}`);
  } else {
    console.error(`❌ unexpected: ${err instanceof Error ? err.message : String(err)}`);
    if (err instanceof Error && err.stack !== undefined) console.error(err.stack);
  }
  process.exit(1);
}
