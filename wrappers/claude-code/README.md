# no-tickets for Claude Code

## Setup

1. Copy `.mcp.json` to your project root (or merge into existing):

```bash
cp .mcp.json /path/to/your/project/.mcp.json
```

2. Set your push token in `.mcp.json`:

```json
"NO_TICKETS_TOKEN": "nt_push_your_token_here"
```

3. (Optional) Add auto-push hook — copy `hooks/` contents to your `.claude/settings.json` hooks section.

## What you get

Three MCP tools available in Claude Code:

- **push** — send project state to the no-tickets dashboard
- **validate** — check `.notickets/` files for format errors
- **status** — verify auth and connection
