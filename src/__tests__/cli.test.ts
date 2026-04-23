import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { parseArgs, runCli } from '../cli.js';

describe('parseArgs', () => {
  it('parses push command with empty args and flags', () => {
    const result = parseArgs(['push']);
    expect(result.command).toBe('push');
    expect(result.args).toEqual([]);
    expect(result.flags).toEqual({});
  });

  it('parses push --dry-run', () => {
    const result = parseArgs(['push', '--dry-run']);
    expect(result.command).toBe('push');
    expect(result.flags['dry-run']).toBe(true);
  });

  it('parses init command', () => {
    const result = parseArgs(['init']);
    expect(result.command).toBe('init');
  });

  it('parses connect with team ID argument', () => {
    const result = parseArgs(['connect', 'team-abc']);
    expect(result.command).toBe('connect');
    expect(result.args[0]).toBe('team-abc');
  });

  it('parses disconnect command', () => {
    const result = parseArgs(['disconnect']);
    expect(result.command).toBe('disconnect');
  });

  it('parses status command', () => {
    const result = parseArgs(['status']);
    expect(result.command).toBe('status');
  });

  it('parses validate command', () => {
    const result = parseArgs(['validate']);
    expect(result.command).toBe('validate');
  });

  it('returns help for no arguments', () => {
    const result = parseArgs([]);
    expect(result).toEqual({ command: 'help', args: [], flags: {} });
  });

  it('returns help for --help flag', () => {
    const result = parseArgs(['--help']);
    expect(result.command).toBe('help');
    expect(result.args).toEqual([]);
    expect(result.flags).toEqual({});
  });

  it('returns help for -h short flag', () => {
    expect(parseArgs(['-h']).command).toBe('help');
  });

  it('returns version for --version flag', () => {
    const result = parseArgs(['--version']);
    expect(result.command).toBe('version');
    expect(result.args).toEqual([]);
    expect(result.flags).toEqual({});
  });

  it('returns version for -v short flag', () => {
    expect(parseArgs(['-v']).command).toBe('version');
  });

  it('treats single-dash args after command as positional', () => {
    const result = parseArgs(['push', '-v']);
    expect(result.command).toBe('push');
    expect(result.args).toEqual(['-v']);
    expect(result.flags).toEqual({});
  });

  it('returns unknown for unrecognized command', () => {
    const result = parseArgs(['foobar']);
    expect(result).toEqual({ command: 'unknown', args: ['foobar'], flags: {} });
  });

  it('collects positional args after command', () => {
    const result = parseArgs(['connect', 'team-abc', 'proj-xyz']);
    expect(result.args).toEqual(['team-abc', 'proj-xyz']);
  });

  it('collects multiple flags', () => {
    const result = parseArgs(['push', '--dry-run', '--verbose']);
    expect(result).toEqual({ command: 'push', args: [], flags: { 'dry-run': true, 'verbose': true } });
  });

  it('skips empty-string args between positionals and flags', () => {
    const result = parseArgs(['push', '', '--dry-run']);
    expect(result).toEqual({ command: 'push', args: [], flags: { 'dry-run': true } });
  });

  it('skips a trailing empty-string arg', () => {
    const result = parseArgs(['connect', 'team-abc', '']);
    expect(result).toEqual({ command: 'connect', args: ['team-abc'], flags: {} });
  });

  it('leaves positional args alone when following a boolean flag', () => {
    // Regression: parseArgs must NOT consume the next arg as a value unless the
    // flag is in the known value-flag allowlist. `dry-run` is a boolean flag,
    // so `somefile` stays a positional.
    const result = parseArgs(['push', '--dry-run', 'somefile']);
    expect(result).toEqual({ command: 'push', args: ['somefile'], flags: { 'dry-run': true } });
  });

  it('parses --project <value> as a string-valued flag', () => {
    const result = parseArgs(['token', 'create', '--project', 'p1', '--label', 'CI']);
    expect(result.command).toBe('token');
    expect(result.args).toEqual(['create']);
    expect(result.flags).toEqual({ project: 'p1', label: 'CI' });
  });

  it('treats a value-flag with no following arg as boolean', () => {
    // `--project` at end of argv with nothing to consume leaves the flag truthy
    // so the handler can surface a "required" error rather than a parse error.
    const result = parseArgs(['token', 'create', '--project']);
    expect(result.flags).toEqual({ project: true });
  });

  it('treats a value-flag followed by another flag as boolean', () => {
    const result = parseArgs(['token', 'create', '--project', '--label', 'CI']);
    expect(result.flags).toEqual({ project: true, label: 'CI' });
  });

  it('produces exactly one flag entry for a trailing value-flag', () => {
    // Guards against off-by-one iteration over argv.
    const result = parseArgs(['token', 'create', '--project', 'p1']);
    expect(Object.keys(result.flags)).toEqual(['project']);
    expect(result.flags['project']).toBe('p1');
  });

  it('skips an empty-string value for a value-flag and falls back to boolean', () => {
    const result = parseArgs(['token', 'create', '--project', '', '--label', 'CI']);
    expect(result.flags).toEqual({ project: true, label: 'CI' });
  });
});

describe('runCli dispatch', () => {
  let logSpy: ReturnType<typeof vi.spyOn>;
  let errSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    logSpy = vi.spyOn(console, 'log').mockImplementation(() => {});
    errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    process.exitCode = undefined;
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('--help prints a Usage line', async () => {
    await runCli(['--help']);
    expect(logSpy).toHaveBeenCalledWith(expect.stringContaining('Usage'));
  });

  it('empty argv prints a Usage line (default help)', async () => {
    await runCli([]);
    expect(logSpy).toHaveBeenCalledWith(expect.stringContaining('Usage'));
  });

  it('--version prints a semver string', async () => {
    await runCli(['--version']);
    expect(logSpy).toHaveBeenCalledOnce();
    expect(logSpy.mock.calls[0]![0] as string).toMatch(/^\d+\.\d+\.\d+/);
  });

  it('init reports not-yet-implemented and exits 1', async () => {
    await runCli(['init']);
    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining('init'));
    expect(process.exitCode).toBe(1);
  });

  it('connect, disconnect fall through to not-implemented', async () => {
    await runCli(['connect']);
    expect(process.exitCode).toBe(1);

    process.exitCode = undefined;
    await runCli(['disconnect']);
    expect(process.exitCode).toBe(1);
  });

  it('unknown command mentions --help and exits 1', async () => {
    await runCli(['foobar']);
    expect(errSpy).toHaveBeenCalledWith(expect.stringContaining('--help'));
    expect(process.exitCode).toBe(1);
  });
});
