# Hermes Documentation -- Project Spec

## Source Codebase

**Location:** `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.Hermes/hermes-agent/`
**Language:** Python 3.11+
**Version:** 0.11.0
**Author:** Nous Research
**License:** MIT

## What Hermes Is

Hermes Agent is a self-improving AI agent that runs on any infrastructure ($5 VPS to cloud), supports any LLM provider, communicates across 10+ messaging platforms (Telegram, Discord, Slack, WhatsApp, Signal, Matrix, Email, etc.), and has a closed learning loop with skills, memory providers, and context engines. It features 40+ pluggable tools, cron scheduling, editor integration via ACP, and RL training trajectory generation.

## Documentation Goal

Create comprehensive documentation that lets any developer understand:
1. How the agent core works (run_agent.py orchestrator, message loop, tool dispatch)
2. How the 40+ tools are registered, discovered, and executed
3. How the multi-platform gateway connects to 10+ messaging services
4. How memory and context management work (providers, compression, search)
5. How the plugin system extends every subsystem
6. How scheduling, skills, and the self-improvement loop work

## Documentation Structure

```
hermes/
├── spec.md                         ← THIS FILE (project tracker)
├── markdown/                       ← Source documentation (viewable on GitHub)
│   ├── README.md                   ← Index / table of contents
│   ├── 00-overview.md              ← What Hermes is, philosophy, capabilities
│   ├── 01-architecture.md          ← Module dependency graph, system layers
│   ├── 02-agent-core.md            ← AIAgent class, message loop, LLM adapters
│   ├── 03-tool-system.md           ← Registry, 40+ tools, dispatch, schemas
│   ├── 04-gateway.md               ← Multi-platform messaging gateway
│   ├── 05-memory-system.md         ← Memory manager, providers, search
│   ├── 06-context-engine.md        ← Context compression, DAG, summarization
│   ├── 07-cli-tui.md               ← CLI entry points, TUI, commands
│   ├── 08-cron.md                  ← Scheduling, jobs, automation
│   ├── 09-plugins.md               ← Plugin architecture, memory/context/image
│   ├── 10-platform-adapters.md     ← Per-platform adapter details
│   └── 11-data-flow.md             ← End-to-end flows with sequence diagrams
├── html/                           ← Rendered HTML (viewable locally + GitHub Pages)
│   ├── index.html                  ← Auto-generated index + navigation
│   ├── styles.css                  ← Shared CSS (dark/light, responsive)
│   └── 00-overview.html ...        ← Auto-generated from markdown
└── build.py                        ← Markdown → HTML (Python stdlib, zero deps)
```

## Tasks

### Phase 1: Core Documentation (Markdown) -- COMPLETE

| # | Task | Status | File |
|---|------|--------|------|
| 1 | Overview: what Hermes is, capabilities, stack | DONE | `00-overview.md` |
| 2 | Architecture: module graph, layers, communication | DONE | `01-architecture.md` |
| 3 | Agent Core: AIAgent, message loop, LLM adapters | DONE | `02-agent-core.md` |
| 4 | Tool System: registry, dispatch, tool categories | DONE | `03-tool-system.md` |
| 5 | Gateway: runner, sessions, message delivery | DONE | `04-gateway.md` |
| 6 | Memory System: manager, providers, built-in memory | DONE | `05-memory-system.md` |
| 7 | Context Engine: compression, DAG, summarization | DONE | `06-context-engine.md` |
| 8 | CLI/TUI: entry points, commands, configuration | DONE | `07-cli-tui.md` |
| 9 | Cron: scheduler, jobs, automation | DONE | `08-cron.md` |
| 10 | Plugins: architecture, types, examples | DONE | `09-plugins.md` |
| 11 | Platform Adapters: per-platform details | DONE | `10-platform-adapters.md` |
| 12 | Data Flow: end-to-end sequences | DONE | `11-data-flow.md` |
| 13 | README index | DONE | `README.md` |

### Phase 2: HTML Rendering -- COMPLETE

| # | Task | Status | File |
|---|------|--------|------|
| 14 | Shared CSS (dark/light, code blocks, diagrams) | DONE | `html/styles.css` |
| 15 | Index page with navigation | DONE | `html/index.html` |
| 16 | Build script (markdown → HTML via Python stdlib) | DONE | `../build.py` |
| 17 | Generate all HTML pages (14 files) | DONE | `html/*.html` |

### Phase 3: Diagrams -- DONE (client-side)

| # | Task | Status | Notes |
|---|------|--------|-------|
| 18 | Mermaid rendering in HTML pages | DONE | Client-side via CDN, no build step |
| 19 | Theme-aware diagram re-rendering | DONE | Re-renders on dark/light toggle |

**Total Mermaid diagrams rendered:** 37 across all pages (3 in architecture, 5 in data-flow, 3 in gateway, etc.)

### Phase 4: Polish -- DONE

| # | Task | Status | Notes |
|---|------|--------|-------|
| 20 | Cross-reference links between documents | DONE | Relative md links auto-converted to HTML |
| 21 | Code snippet rendering | DONE | Escaped, syntax-tagged, scrollable |
| 22 | Mobile-responsive HTML layout | DONE | Media query at 600px in styles.css |

## Build System

**Script:** `documentation/build.py` (shared with Pi)
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
# Build both projects
cd documentation && python3 build.py

# Build just Hermes
python3 build.py hermes

# Build just Pi
python3 build.py pi
```

**Rebuild:** Run the same command. It overwrites existing HTML files. Idempotent.

## File Counts

| Type | Count |
|------|-------|
| Markdown source files | 13 |
| Generated HTML files | 14 (13 docs + 1 index) |
| CSS files | 1 (shared) |
| Total HTML output | 14 files + styles.css |

## Expected Outcome

A developer unfamiliar with Hermes can:
1. Read the overview and understand what Hermes does in 5 minutes
2. Read the architecture doc and understand how the modules fit together
3. Deep-dive into any subsystem (tools, gateway, memory, plugins)
4. Follow data flow diagrams to understand what happens when a message arrives
5. Understand how to extend Hermes with custom tools, memory providers, and plugins
6. View the documentation as rendered HTML locally or deploy to GitHub Pages

## Resume Point

All phases complete. To continue work:
1. Add new content to `markdown/*.md`, then run `python3 build.py hermes` to regenerate HTML
2. Edit `html/styles.css` to adjust styling, then commit
3. For GitHub Pages deployment, push the `html/` directory and configure Pages to serve from it
