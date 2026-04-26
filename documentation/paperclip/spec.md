# Paperclip Documentation -- Project Spec

## Source Codebase

**Location:** `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AIApps/src.paperclip/paperclip/`
**Language:** TypeScript (Node.js, pnpm workspaces monorepo)
**License:** MIT
**Author:** Paperclip
**Runtime:** Node.js 20+, pnpm 9.15+

## What Paperclip Is

Paperclip is an open-source orchestration platform for zero-human companies. A Node.js server and React UI that orchestrates teams of AI agents to run a business. Bring your own agents (OpenClaw, Claude Code, Codex, Cursor, Bash, HTTP, Pi, Hermes), assign goals, and track work and costs from one dashboard. Features org charts, budgets, governance, goal alignment, heartbeats, ticket system with full tracing, and multi-company support with data isolation.

## Documentation Goal

Create comprehensive documentation that lets any developer understand:

1. What Paperclip is and the problems it solves (comparison with agent frameworks and task managers)
2. How the project is structured -- server, UI, internal packages, sub-projects
3. How orchestration works -- org charts, heartbeats, task delegation, agent adapters
4. How goal alignment and budget enforcement work -- hierarchical tasks, board governance
5. How agent integration works -- BYOA, adapter types, heartbeat protocol, skill system
6. How the ticket system works -- task hierarchy, atomic checkout, audit log, tracing
7. What the sub-projects are -- Clipmart, PR Reviewer, Companies Tool, Paperclip Website
8. How to develop, test, deploy, and contribute to Paperclip

## Documentation Structure

```
paperclip/
├── spec.md                         ← THIS FILE (project tracker)
├── markdown/                       ← Source documentation (viewable on GitHub)
│   ├── README.md                   ← Index / table of contents
│   ├── 00-overview.md              ← What Paperclip is, philosophy, comparison table
│   ├── 01-architecture.md          ← Project structure, packages, tech stack, data flow
│   ├── 02-orchestration.md         ← Org charts, roles, heartbeats, task delegation
│   ├── 03-goals-budgets.md         ← Goal alignment, budget enforcement, governance
│   ├── 04-agents-integration.md    ← BYOA, adapter types, integration levels, heartbeats
│   ├── 05-ticket-system.md         ← Task hierarchy, audit log, tracing, atomic checkout
│   ├── 06-subprojects.md           ← Clipmart, PR Reviewer, Companies Tool, Website
│   └── 07-development.md           ← Setup, roadmap, telemetry, contributing
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
| 1 | README index/catalog | DONE | `README.md` |
| 2 | Overview: what Paperclip is, philosophy, comparison table | DONE | `00-overview.md` |
| 3 | Architecture: project structure, packages, tech stack, data flow | DONE | `01-architecture.md` |
| 4 | Orchestration: org charts, roles, heartbeats, task delegation | DONE | `02-orchestration.md` |
| 5 | Goals and Budgets: goal alignment, budget enforcement, governance | DONE | `03-goals-budgets.md` |
| 6 | Agents Integration: BYOA, adapter types, integration levels, heartbeats | DONE | `04-agents-integration.md` |
| 7 | Ticket System: task hierarchy, audit log, tracing, atomic checkout | DONE | `05-ticket-system.md` |
| 8 | Sub-projects: Clipmart, PR Reviewer, Companies Tool, Website | DONE | `06-subprojects.md` |
| 9 | Development: setup, roadmap, telemetry, contributing | DONE | `07-development.md` |

### Phase 2: HTML Rendering -- COMPLETE

| # | Task | Status | File |
|---|------|--------|------|
| 10 | Shared CSS (dark/light, code blocks, diagrams) | DONE | `html/styles.css` |
| 11 | Index page with navigation | DONE | `html/index.html` |
| 12 | Build script (markdown to HTML via Python stdlib) | DONE | `../build.py` |
| 13 | Generate all HTML pages (9 files) | DONE | `html/*.html` |

### Phase 3: Diagrams -- DONE (client-side)

| # | Task | Status | Notes |
|---|------|--------|-------|
| 14 | Mermaid rendering in HTML pages | DONE | Client-side via CDN, no build step |
| 15 | Theme-aware diagram re-rendering | DONE | Re-renders on dark/light toggle |

### Phase 4: Polish -- DONE

| # | Task | Status | Notes |
|---|------|--------|-------|
| 16 | Cross-reference links between documents | DONE | Relative md links auto-converted to HTML |
| 17 | Code snippet rendering | DONE | Escaped, syntax-tagged, scrollable |
| 18 | Mobile-responsive HTML layout | DONE | Media query at 600px in styles.css |

## Build System

**Script:** `documentation/build.py` (shared with Pi, Hermes)
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

# Build just Paperclip
python3 build.py paperclip

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
| Generated HTML files | 10 (9 docs + 1 index) |
| CSS files | 1 (shared) |
| Total HTML output | 9 files + styles.css |

## Expected Outcome

A developer unfamiliar with Paperclip can:
1. Read the overview and understand what Paperclip does in 5 minutes
2. Read the architecture doc and understand how the packages and sub-projects fit together
3. Deep-dive into any subsystem (orchestration, agents, ticket system, governance)
4. Follow data flow diagrams to understand what happens when an agent is woken up or a task is assigned
5. Understand how to extend Paperclip with custom agents, adapters, and plugins
6. View the documentation as rendered HTML locally or deploy to GitHub Pages

## Resume Point

All phases complete. To continue work:
1. Add new content to `markdown/*.md`, then run `python3 build.py paperclip` to regenerate HTML
2. Edit `html/styles.css` to adjust styling, then commit
3. For GitHub Pages deployment, push the `html/` directory and configure Pages to serve from it
