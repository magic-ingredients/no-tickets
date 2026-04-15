import { describe, it, expect } from 'vitest';
import { parseArgs } from '../cli.js';

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
    expect(result.command).toBe('help');
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
    expect(result.command).toBe('unknown');
    expect(result.args[0]).toBe('foobar');
  });

  it('collects positional args after command', () => {
    const result = parseArgs(['connect', 'team-abc', 'proj-xyz']);
    expect(result.args).toEqual(['team-abc', 'proj-xyz']);
  });

  it('collects multiple flags', () => {
    const result = parseArgs(['push', '--dry-run', '--verbose']);
    expect(result.flags['dry-run']).toBe(true);
    expect(result.flags['verbose']).toBe(true);
  });
});
