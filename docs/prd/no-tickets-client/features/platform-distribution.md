---
id: platform-distribution
prd_id: no-tickets-client
number: 8
title: Platform Distribution — MCP Wrappers
status: completed
created: 2026-04-17
updated: 2026-04-22
---

# Feature: Platform Distribution — MCP Wrappers

## Description

Platform-specific wrappers that package the core MCP server (Feature 5) for each ecosystem's plugin format. The core server is built once, wrapped N times. All wrappers reference `npx -y @magic-ingredients/no-tickets` for the MCP server.

The wrappers are thin — they configure transport and auth for their platform, nothing else. PM workflow skills (create epic, break down feature, etc.) are NOT in these wrappers — they belong in tiny-brain.

### Architecture

```
@magic-ingredients/no-tickets (this repo)
├── src/mcp/                      ← Core MCP server (Feature 5) — push, validate, status
├── wrappers/
│   ├── claude-code/              ← .claude-plugin/ (MCP + post-push hook)
│   ├── claude-desktop/           ← .mcpb (Desktop Extension zip)
│   ├── cursor/                   ← .cursor-plugin/ (MCP)
│   ├── chatgpt/                  ← Apps SDK (MCP) + Custom GPT
│   ├── gemini/                   ← gemini-extension.json (MCP)
│   ├── copilot/                  ← mcp.json config
│   ├── windsurf/                 ← mcp_config.json snippet
│   └── continue/                 ← block.yaml
```

## Acceptance Criteria

- [ ] Claude Code plugin ships with MCP server and optional post-tool push hook
- [ ] Claude Desktop extension installs via double-click .mcpb file
- [ ] Cursor plugin available on cursor.com/marketplace
- [ ] ChatGPT custom GPT available in GPT Store
- [ ] All platform wrappers reference the same core MCP server
- [ ] Config snippets for Copilot, Windsurf, Continue.dev

## Tasks

### 1. Build Claude Code plugin wrapper
status: completed
commitSha: 6deb49b

### 2. Build Claude Desktop extension
status: completed
commitSha: 6deb49b

### 3. Build Cursor plugin wrapper
status: completed
commitSha: 6deb49b

### 4. Build ChatGPT integration
status: completed
commitSha: 6deb49b

### 5. Build Gemini CLI extension
status: completed
commitSha: 6deb49b

### 6. Create config snippets for Copilot, Windsurf, Continue.dev
status: completed
commitSha: 6deb49b

## Dependencies

- Feature 5 (MCP Server) — core server must be working
- Feature 6 (OSS Launch) — package must be published to npm

## Testing Strategy

### Manual Testing
- Install each wrapper end-to-end on its target platform
- Verify push, validate, status tools respond correctly via each wrapper
