# OpenPencil -- Overview

## What OpenPencil Is

OpenPencil is an open-source design editor that opens `.fig` and `.pen` files, includes built-in AI powered by 100+ design tools, and ships as a programmable toolkit with a headless Vue SDK for building custom editors.

It is designed around three principles:

1. **Open and scriptable** -- Every operation is accessible via CLI, MCP, or code. No black boxes.
2. **Runs anywhere** -- Desktop app (macOS, Windows, Linux) via Tauri v2, or in the browser as a PWA.
3. **Your data stays yours** -- No accounts, no cloud lock-in, no telemetry. P2P collaboration goes directly browser-to-browser.

> **Status:** Active development. Not ready for production use.
>
> **License:** MIT

## Capabilities

```
┌─────────────────────────────────────────────────────────────────┐
│                        OPENPENCIL                                │
│                                                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │ File Format │  │ Rendering   │  │ Layout      │             │
│  │ .fig reader │  │ Skia WASM   │  │ Yoga WASM   │             │
│  │ .pen native │  │ CanvasKit   │  │ Flex + Grid │             │
│  │ Kiwi binary │  │ GPU-accel   │  │ Fork w/grid │             │
│  │ Zstd+ZIP    │  │ SVG export  │  │             │             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
│                                                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │ AI (100+    │  │ MCP Server  │  │ CLI         │             │
│  │ tools)      │  │ 100+ tools  │  │ tree/query  │             │
│  │ Anthropic   │  │ Stdio+HTTP  │  │ export/lint │             │
│  │ OpenAI      │  │ RPC bridge  │  │ analyze/eval│             │
│  │ Google AI   │  │ Agent integration         │             │
│  │ OpenRouter  │  └─────────────┘  └─────────────┘             │
│  └─────────────┘                                                │
│                                                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │ Collab      │  │ Vue SDK     │  │ Lint &      │             │
│  │ WebRTC P2P  │  │ Headless    │  │ Analyze     │             │
│  │ Yjs CRDT    │  │ Composables │  │ 18 rules    │             │
│  │ Presence    │  │ Components  │  │ Tokens      │             │
│  │ Follow mode │  │ Custom edits│  │ Clusters    │             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
└─────────────────────────────────────────────────────────────────┘
```

## Key Differentiators

### Native .fig Compatibility

Figma is a closed platform that actively fights programmatic access. Their MCP server is read-only. OpenPencil reads `.fig` files natively by implementing the Kiwi binary protocol -- the same format Figma uses internally. This means:

- Open `.fig` files without Figma installed
- Copy & paste nodes between Figma and OpenPencil
- Export selections back to `.fig` format
- Script everything via CLI, MCP, or `eval`

### AI-Native Design Tools

The built-in AI assistant has 100+ tools that can create shapes, set fills and strokes, manage auto-layout, work with components and variables, run boolean operations, analyze design tokens, and export assets. Connect any LLM provider (Anthropic, OpenAI, Google AI, OpenRouter, Z.ai, MiniMax).

### Programmable at Every Layer

- **Headless CLI** -- Inspect, query, export, lint, and analyze design files from the terminal
- **MCP Server** -- 100+ tools for AI agents (Claude Code, Cursor, Windsurf) to manipulate designs
- **Vue SDK** -- Headless components and composables for building custom editors
- **Figma Plugin API** -- `eval` command gives you the full Figma Plugin API in scripts

### Real-Time Collaboration Without a Server

Peers connect directly via WebRTC. No signaling server relays your data, no account required. Document state uses Yjs CRDT for automatic conflict resolution.

## Technology Stack

| Concern | Choice |
|---------|--------|
| UI Framework | Vue 3, Reka UI, Tailwind CSS 4 |
| Rendering | Skia CanvasKit WASM |
| Layout | Yoga WASM (flex + grid via [open-pencil/yoga fork](https://github.com/open-pencil/yoga/tree/grid)) |
| Desktop | Tauri v2 (Rust + system webview) |
| File Format | Kiwi binary + Zstd compression + ZIP |
| Collaboration | Trystero (WebRTC P2P) + Yjs (CRDT) |
| AI/MCP | Vercel AI SDK, MCP SDK, Hono |
| Linting | oxlint + tsgolint (type-aware) |
| Testing | Playwright (E2E), Bun (unit) |
| Package Manager | Bun (workspaces) |

## Entry Points

| Command | What It Does |
|---------|-------------|
| `open-pencil tree` | Inspect design file node tree |
| `open-pencil query` | XPath queries on design files |
| `open-pencil export` | Render to PNG/JPG/WEBP/SVG/JSX/FIG |
| `open-pencil lint` | Check naming, layout, accessibility |
| `open-pencil analyze` | Audit colors, typography, spacing, clusters |
| `open-pencil eval` | Execute Figma Plugin API scripts |
| `openpencil-mcp` | Start MCP server (stdio) |
| `openpencil-mcp-http` | Start MCP server (HTTP) |
| Desktop app | Full visual editor (Tauri v2) |
| Web app (PWA) | Browser-based editor |

## Project Structure

```
packages/
  core/           @open-pencil/core -- Engine (scene graph, renderer, layout, file formats, tools)
  vue/            @open-pencil/vue  -- Headless Vue SDK for custom editors
  cli/            @open-pencil/cli  -- Headless CLI tool
  mcp/            @open-pencil/mcp  -- MCP server (stdio + HTTP)
  docs/           Documentation site (openpencil.dev)
src/              Vue app (components, composables, stores)
desktop/          Tauri v2 configuration (Rust)
tests/            E2E (188 tests) + unit (764 tests)
```

## Why It Exists

Figma dominates the design tool market but remains a walled garden. Their files are in a proprietary binary format, their MCP server is read-only, and they have killed community workarounds (CDP access in Figma Desktop 126). OpenPencil provides the open alternative:

- **MIT licensed** -- Use, modify, distribute freely
- **Open file format** -- `.pen` files are documented and parseable
- **Scriptable** -- Every operation available via CLI, MCP, or code
- **Self-hosted** -- Your data never leaves your machine
- **Collaborative** -- P2P WebRTC, no central server needed

## See Also

- [Architecture](01-architecture.md) -- Package dependency graph, system layers
- [Core Engine](02-core-engine.md) -- Scene graph, renderer, file formats, tools
- [CLI](03-cli.md) -- Headless CLI commands and usage
- [AI & MCP](04-ai-mcp.md) -- AI chat, MCP server, coding agents
- [Collaboration](05-collaboration.md) -- WebRTC P2P, presence, follow mode
- [Vue SDK](06-vue-sdk.md) -- Headless Vue SDK for custom editors
- [Development](07-development.md) -- Setup, quality gates, roadmap
