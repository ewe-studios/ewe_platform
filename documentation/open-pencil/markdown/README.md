# OpenPencil Documentation

Comprehensive documentation for [OpenPencil](https://github.com/open-pencil/open-pencil) -- the open-source design editor that opens `.fig` and `.pen` files.

## Documentation Catalog

| # | Document | Description |
|---|----------|-------------|
| 1 | [Overview](00-overview.md) | What OpenPencil is, philosophy, capabilities, tech stack, and why it exists |
| 2 | [Architecture](01-architecture.md) | Package dependency graph, system layers, communication patterns, key abstractions |
| 3 | [Core Engine](02-core-engine.md) | Scene graph, Skia CanvasKit renderer, Yoga layout, file formats (Kiwi/.fig/.pen), 100+ tool registry, linter, XPath engine |
| 4 | [CLI](03-cli.md) | Headless CLI commands: tree, find, node, info, query, export, convert, lint, analyze, eval, variables, pages, selection |
| 5 | [AI & MCP](04-ai-mcp.md) | Built-in AI chat, MCP server (100+ tools, stdio + HTTP), coding agent integration (Claude Code, Codex, Gemini CLI), AI agent skill |
| 6 | [Collaboration](05-collaboration.md) | WebRTC P2P real-time collaboration, Yjs CRDT, presence, follow mode, room lifecycle |
| 7 | [Vue SDK](06-vue-sdk.md) | Headless Vue SDK for custom editors: composables, property controls, color picker, layer tree, variables editor, i18n |
| 8 | [Development](07-development.md) | Setup, quality gates, desktop builds, PWA, testing, roadmap, contributing |

## Quick Links

- [GitHub Repository](https://github.com/open-pencil/open-pencil)
- [Try it online](https://app.openpencil.dev/demo)
- [Documentation site](https://openpencil.dev)
- [MCP tool reference](https://openpencil.dev/reference/mcp-tools)
- [Vue SDK docs](https://openpencil.dev/programmable/sdk/)

## Status

**Active development. Not ready for production use.** MIT licensed.
