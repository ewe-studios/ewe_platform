# Mastra -- Official Documentation Site (docs/)

## Overview

Mastra ships a **Docusaurus-based documentation site** at `mastra/docs/`. This is the source for the public documentation at [mastra.ai/docs](https://mastra.ai/docs). Unlike the markdown documentation in this project (which provides deep technical analysis), the official docs are user-facing guides for building with Mastra.

**Key insight:** The docs site uses Docusaurus Faster (experimental Docusaurus v4), TailwindCSS, and a custom plugin system. It's structured into 5 documentation collections with separate routing.

## Documentation Architecture

```mermaid
flowchart TD
    DOCS[mastra/docs/] --> DCONF[docusaurus.config.ts]

    DCONF --> DOCS_COL["docs/ (routeBasePath: /docs)"]
    DCONF --> GUIDES_COL["guides/ (routeBasePath: /guides)"]
    DCONF --> MODELS_COL["models/ (routeBasePath: /models)"]
    DCONF --> REF_COL["reference/ (routeBasePath: /reference)"]
    DCONF --> LEARN[learn/ -- Learn section]

    DOCS_COL --> D_SECTIONS[18 sections]
    D_SECTIONS --> D1[getting-started, agents, workflows, memory]
    D_SECTIONS --> D2[streaming, MCP, server, deployment]
    D_SECTIONS --> D3[observability, evals, RAG, voice, browser]
    D_SECTIONS --> D4[editor, workspace, studio, build-with-ai]

    GUIDES_COL --> G_SECTIONS[7 sections]
    G_SECTIONS --> G1[getting-started: Next.js, React, Astro, SvelteKit, Nuxt, Express, Hono, Electron]
    G_SECTIONS --> G2[guide: tutorials (chef-michel, stock-agent, etc.)]
    G_SECTIONS --> G3[build-your-ui: AI SDK, CopilotKit, Assistant UI]
    G_SECTIONS --> G4[deployment: Vercel, AWS, Cloudflare, Azure, etc.]
    G_SECTIONS --> G5[migrations: v1.0, Cloud, Network→Supervisor]

    DCONF --> PLUGINS[Plugins]
    PLUGINS --> P1[docusaurus-plugin-learn -- powers Learn section]
    PLUGINS --> P2[docusaurus-plugin-llms-txt -- generates llms.txt]
    PLUGINS --> P3[tailwind-plugin -- CSS processing]
```

## 5 Documentation Collections

### 1. Docs (`/docs`) -- Main Documentation

| Section | Content |
|---------|---------|
| **getting-started** | Project structure, manual install, build with AI |
| **studio** | Studio overview, deployment, auth, observability |
| **agents** | Tools, structured output, supervisor agents, processors, guardrails, agent approval, voice, channels, networks (deprecated) |
| **memory** | Storage, message history, observational memory, working memory, semantic recall, memory processors |
| **workflows** | Workflow state, control flow, agents & tools, snapshots, suspend & resume, human-in-the-loop, time travel, error handling |
| **editor** | Visual editor for tools, prompts (new) |
| **streaming** | Streaming overview, events, tool streaming, workflow streaming |
| **MCP** | MCP overview, publishing MCP servers |
| **workspace** | Filesystem, sandbox, LSP inspection, skills, search/indexing |
| **browser** | AgentBrowser, Stagehand (new) |
| **server** | Server adapters, custom adapters, middleware, request context, custom API routes, Mastra client, auth (11 providers) |
| **deployment** | Mastra server, monorepo, cloud providers, web framework, workflow runners |
| **observability** | Logging, tracing (12 exporters), metrics, processors (sensitive data filter) |
| **evals** | Built-in scorers, custom scorers, CI integration, datasets |
| **mastra-platform** | Platform overview, configuration (new) |
| **RAG** | Chunking/embedding, vector databases, retrieval, GraphRAG |
| **voice** | Text-to-speech, speech-to-text, speech-to-speech |
| **build-with-ai** | Skills, MCP docs server |

### 2. Guides (`/guides`) -- Tutorials and Quickstarts

| Section | Content |
|---------|---------|
| **getting-started** | Quickstart + 8 framework integrations: Next.js, React, Astro, SvelteKit, Nuxt, Express, Hono, Electron |
| **concepts** | Multi-agent systems |
| **agent-frameworks** | AI SDK integration |
| **build-your-ui** | AI SDK UI, CopilotKit, Assistant UI |
| **deployment** | 9 deployment targets: EC2, Lambda, Azure, Cloudflare, Digital Ocean, Inngest, Mastra Platform, Netlify, Vercel |
| **tutorials** | 13 tutorials: Chef Michel (agents), Stock Agent (tools), Web Search, Firecrawl, AI Recruiter (workflows), Research Assistant (RAG), Notes MCP Server, Research Coordinator (supervisor agents), Dev Assistant (workspace), Code Review Bot (skills), Docs Manager (filesystem), WhatsApp Bot, GitHub Actions PR Description |
| **migrations** | v1.0 migration (17 sub-docs), Mastra Cloud migration, Network→Supervisor, VNext→Standard APIs, AI SDK v4→v5 |

### 3. Models (`/models`) -- Model Provider Documentation

Separate Docusaurus plugin instance at `/models` route. Documents supported model providers and their configurations.

### 4. Reference (`/reference`) -- API Reference

Separate Docusaurus plugin instance at `/reference` route. API reference documentation for Mastra packages.

### 5. Learn (`learn/` internal) -- Interactive Learning

Powered by `docusaurus-plugin-learn`. Contains:
- Course content structure
- Learning pages
- Hooks and utilities

## Key Architecture Details

### Docusaurus Faster

The docs use `@docusaurus/faster` (experimental Docusaurus v4), enabled via:

```typescript
future: {
  experimental_faster: true,
}
```

This provides faster builds with Rspack instead of Webpack.

### Custom Plugins

| Plugin | Purpose |
|--------|---------|
| **docusaurus-plugin-learn** | Generates the Learn section with course content, pages, and interactive elements |
| **docusaurus-plugin-llms-txt** | Generates `llms.txt` for LLM consumption -- a single file summarizing the entire site |
| **tailwind-plugin** | Integrates TailwindCSS for styling |

### npm2yarn Remark Plugin

All docs support automatic conversion of `npm` commands to `pnpm`, `yarn`, and `bun`:

```typescript
remarkPlugins: [[require('@docusaurus/remark-plugin-npm2yarn'), {
  sync: true,
  converters: ['pnpm', 'yarn', 'bun']
}]]
```

### Algolia Search

The docs integrate Algolia DocSearch for site-wide search, with environment-variable-based configuration.

### Internationalization

The docs support multiple languages. Currently:
- `src/content/en/` -- English (full content)
- `src/content/ja/` -- Japanese

## Comparison: Official Docs vs This Project's Docs

| Aspect | Official docs/ | documentation/mastra/ |
|--------|---------------|----------------------|
| **Audience** | Users building with Mastra | Developers understanding Mastra internals |
| **Format** | Docusaurus (MDX + React) | Static HTML from markdown |
| **Depth** | API reference, tutorials, quickstarts | Deep architecture, data flow, comparison |
| **Source** | `mastra/docs/src/content/` | `documentation/mastra/markdown/` |
| **Code** | Usage examples, copy-paste snippets | Actual source code from `mastra/packages/` |
| **Purpose** | "How to use Mastra" | "How Mastra works" |

## Related Documents

- [01-architecture.md](./01-architecture.md) -- Mastra package map and monorepo structure
- [15-ecosystem.md](./15-ecosystem.md) -- Production services, applications, and templates
- [16-plugin-ecosystem.md](./16-plugin-ecosystem.md) -- Sub-packages within mastra/

## Source Paths

```
mastra/docs/
├── docusaurus.config.ts              ← Main Docusaurus config (Faster, 5 collections)
├── src/
│   ├── content/en/
│   │   ├── docs/                     ← Main docs (18 sections, /docs route)
│   │   │   ├── agents/               ← Agent docs (tools, processors, guardrails, etc.)
│   │   │   ├── workflows/            ← Workflow docs (suspend/resume, HITL, time travel)
│   │   │   ├── memory/               ← Memory docs (OM, working memory, semantic recall)
│   │   │   ├── server/               ← Server + auth (11 providers)
│   │   │   ├── observability/        ← Tracing, logging, metrics, exporters
│   │   │   └── ...                   ← 13 more sections
│   │   ├── guides/                   ← Tutorials (13 guides, /guides route)
│   │   │   ├── getting-started/      ← 8 framework quickstarts
│   │   │   ├── guide/                ← Tutorial walkthroughs
│   │   │   ├── build-your-ui/        ← UI framework integrations
│   │   │   ├── deployment/           ← 9 deployment targets
│   │   │   └── migrations/           ← v1.0 + 4 other migration guides
│   │   ├── models/                   ← Model provider docs (/models route)
│   │   └── reference/                ← API reference (/reference route)
│   ├── learn/                        ← Interactive learning section
│   ├── plugins/                      ← Custom Docusaurus plugins
│   ├── pages/                        ← Custom pages (kitchen-sink.mdx)
│   └── theme/                        ← Custom theme (Prism syntax highlighting)
└── static/                           ← Static assets (images, favicon)
```
