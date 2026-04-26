# OpenPencil Documentation -- Project Spec

## Source Codebase

**Location:** `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AIApps/open-pencil/`
**Language:** TypeScript + Rust (Tauri v2)
**Version:** 0.11.8
**Author:** open-pencil contributors
**License:** MIT

## What OpenPencil Is

OpenPencil is an open-source design editor that opens `.fig` and `.pen` files, includes built-in AI powered by 100+ design tools, and ships as a programmable toolkit with a headless Vue SDK for building custom editors. It features a Vue 3 + Tauri v2 desktop app, PWA web app, Skia CanvasKit WASM rendering, Yoga WASM layout, WebRTC P2P real-time collaboration, MCP server with 100+ tools, and a headless CLI. The desktop app is ~7 MB.

## Documentation Goal

Create comprehensive documentation that lets any developer understand:
1. What OpenPencil does and its philosophy of openness and programmability
2. How the monorepo is structured (core engine + CLI + MCP + Vue SDK)
3. How the rendering pipeline works (Skia CanvasKit WASM + Yoga layout)
4. How the file formats work (.fig Kiwi binary, .pen ZIP, export formats)
5. How the 100+ AI tools are defined and exposed
6. How the CLI works headlessly on design files
7. How the MCP server bridges AI agents to the editor
8. How WebRTC P2P collaboration works
9. How the Vue SDK enables custom editors
10. How to set up, test, and build the project

## Documentation Structure

```
open-pencil/
├── spec.md                         ← THIS FILE (project tracker)
├── markdown/                       ← Source documentation (viewable on GitHub)
│   ├── README.md                   ← Index / table of contents
│   ├── 00-overview.md              ← What OpenPencil is, philosophy, capabilities
│   ├── 01-architecture.md          ← Package dependency graph, system layers
│   ├── 02-core-engine.md           ← Scene graph, renderer, file formats, tools
│   ├── 03-cli.md                   ← Headless CLI commands and usage
│   ├── 04-ai-mcp.md                ← AI chat, MCP server, coding agents
│   ├── 05-collaboration.md         ← WebRTC P2P, presence, follow mode
│   ├── 06-vue-sdk.md               ← Headless Vue SDK for custom editors
│   └── 07-development.md           ← Setup, quality gates, desktop builds, roadmap
├── html/                           ← Rendered HTML (viewable locally + GitHub Pages)
│   ├── index.html                  ← Auto-generated index + navigation
│   ├── styles.css                  ← Shared CSS (dark/light, responsive)
│   └── 00-overview.html ...        ← Auto-generated from markdown
└── build.py                        ← Markdown → HTML (Python stdlib, zero deps, in parent dir)
```

## Tasks

### Phase 1: Core Documentation (Markdown) -- COMPLETE

| # | Task | Status | File |
|---|------|--------|------|
| 1 | Overview: what OpenPencil is, capabilities, stack | DONE | `00-overview.md` |
| 2 | Architecture: package graph, layers, communication | DONE | `01-architecture.md` |
| 3 | Core Engine: scene graph, renderer, layout, formats, tools | DONE | `02-core-engine.md` |
| 4 | CLI: all commands, usage examples, JSON output | DONE | `03-cli.md` |
| 5 | AI & MCP: chat, MCP server, coding agents, skill | DONE | `04-ai-mcp.md` |
| 6 | Collaboration: WebRTC P2P, CRDT, presence, follow mode | DONE | `05-collaboration.md` |
| 7 | Vue SDK: composables, components, i18n, custom editors | DONE | `06-vue-sdk.md` |
| 8 | Development: setup, quality gates, builds, roadmap | DONE | `07-development.md` |
| 9 | README index | DONE | `README.md` |

### Phase 2: HTML Rendering -- COMPLETE

| # | Task | Status | File |
|---|------|--------|------|
| 10 | Shared CSS (dark/light, code blocks, diagrams) | DONE | `html/styles.css` |
| 11 | Index page with navigation | DONE | `html/index.html` |
| 12 | Build script (markdown → HTML via Python stdlib) | DONE | `../build.py` |
| 13 | Generate all HTML pages (8 docs + 1 index) | DONE | `html/*.html` |

### Phase 3: Diagrams -- DONE (client-side)

| # | Task | Status | Notes |
|---|------|--------|-------|
| 14 | Mermaid rendering in HTML pages | DONE | Client-side via CDN, no build step |
| 15 | Theme-aware diagram re-rendering | DONE | Re-renders on dark/light toggle |

**Total Mermaid diagrams rendered:** 9 across all pages (2 in architecture, 1 in core engine data flow, 1 in AI sequence diagram, 1 in MCP architecture, 2 in collaboration, 1 in Vue SDK, 1 in architecture overview)

### Phase 4: Polish -- DONE

| # | Task | Status | Notes |
|---|------|--------|-------|
| 16 | Cross-reference links between documents | DONE | Relative md links auto-converted to HTML |
| 17 | Code snippet rendering | DONE | Escaped, syntax-tagged, scrollable |
| 18 | Mobile-responsive HTML layout | DONE | Media query at 600px in styles.css |

## Build System

**Script:** `documentation/build.py` (shared with Pi and Hermes)
**Dependencies:** None (Python 3.12+ stdlib only)
**Features:**
- Converts all markdown to HTML with tables, code blocks, headings, lists, links, blockquotes
- Extracts titles from frontmatter or first `#` heading
- Generates index pages with all document links
- Embeds Mermaid client-side loader (CDN, conditional -- only loads when `.mermaid` blocks exist)
- Embeds dark/light theme toggle with `localStorage` persistence
- Generates prev/next navigation between pages
- Copies shared `styles.css` on first run

**Usage:**
```bash
# Build all projects
cd documentation && python3 build.py

# Build just OpenPencil
python3 build.py open-pencil

# Build just Pi
python3 build.py pi

# Build just Hermes
python3 build.py hermes
```

**Rebuild:** Run the same command. It overwrites existing HTML files. Idempotent.

## File Counts

| Type | Count |
|------|-------|
| Markdown source files | 9 |
| Generated HTML files | 10 (8 docs + 1 index + styles.css) |
| CSS files | 1 (shared) |

## Expected Outcome

A developer unfamiliar with OpenPencil can:
1. Read the overview and understand what OpenPencil does in 5 minutes
2. Read the architecture doc and understand how the monorepo packages fit together
3. Deep-dive into any subsystem (core engine, CLI, MCP, Vue SDK, collaboration)
4. Follow data flow diagrams to understand how rendering and AI tool execution work
5. Understand how to extend OpenPencil with custom tools, Vue components, or MCP integrations
6. View the documentation as rendered HTML locally or deploy to GitHub Pages

## Resume Point

All phases complete. To continue work:
1. Add new content to `markdown/*.md`, then run `python3 build.py open-pencil` to regenerate HTML
2. Edit `html/styles.css` to adjust styling, then commit
3. For GitHub Pages deployment, push the `html/` directory and configure Pages to serve from it
