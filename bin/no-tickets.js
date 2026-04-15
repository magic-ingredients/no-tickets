#!/usr/bin/env node

/**
 * @magic-ingredients/no-tickets
 *
 * Single entry point with automatic mode detection:
 * - MCP server mode: when stdin is piped (launched by MCP client)
 * - CLI mode: when run interactively with arguments
 * - Help mode: when run interactively without arguments
 */

const args = process.argv.slice(2);
const isInteractive = process.stdin.isTTY ?? false;

if (args.length === 0 && !isInteractive) {
  // Stdin is piped, no args → MCP client launched us
  const { startMcpServer } = await import('../dist/mcp/create-server.js');
  startMcpServer();
} else if (args.length === 0) {
  // Interactive terminal, no args → show help
  console.log(`no-tickets — ticketless project management for AI teams

Usage:
  npx no-tickets <command> [options]

Commands:
  init              Authenticate and connect to a project
  push              Push local state to dashboard
  push --ci         Push from CI (authoritative scores)
  push --dry-run    Preview what would be pushed
  status            Show connection and auth status
  validate          Check .notickets/ files against spec
  token create      Create a push token for CI
  token list        List push tokens
  token revoke      Revoke a push token

Options:
  --help            Show this help message
  --version         Show version

Documentation: https://docs.no-tickets.com
`);
} else {
  // Has args → CLI mode
  const { runCli } = await import('../dist/cli.js');
  runCli(args);
}
