---
title: "Pi Extensions -- pi-mcp-adapter"
---

# pi-mcp-adapter

**Use MCP servers with Pi without burning your context window.**

pi-mcp-adapter gives Pi access to MCP (Model Context Protocol) servers while keeping the context cost minimal. Instead of loading hundreds of tool definitions into the context window (10k+ tokens per server), a single proxy tool (~200 tokens) provides on-demand access.

## The Problem

Tool definitions are verbose. A single MCP server can burn 10k+ tokens. Connect a few servers and you've burned half your context window before the conversation starts.

The alternative recommendation is to "skip MCP entirely, write simple CLI tools instead." But the MCP ecosystem has useful tools -- databases, browsers, APIs -- that aren't available as CLI tools.

## The Solution

One proxy tool (~200 tokens) instead of hundreds of tool definitions. The agent discovers what it needs on-demand. Servers only start when you actually use them.

## How It Works

```
Pi Agent ──► MCP Adapter (1 tool, ~200 tokens)
                  │
                  ├── Server A (starts on first use)
                  ├── Server B (starts on first use)
                  └── Server C (starts on first use)
```

The adapter acts as a gateway. When the agent needs a capability, the adapter starts the appropriate MCP server, calls the tool, and returns the result. Unused servers never start, never cost tokens.

## Install

```bash
pi install npm:pi-mcp-adapter
```

Configure MCP servers in Pi's settings. The adapter handles discovery and lazy loading automatically.

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-mcp-adapter` |
| Context cost | ~200 tokens (single proxy tool) |
| Pattern | Lazy-loaded MCP server gateway |
