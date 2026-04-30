# Pi Documentation -- Project Spec

## Source Codebase

**Location:** `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.Pi/pi-mono/`
**Language:** TypeScript (monorepo, npm workspaces)
**Version:** 0.68.1
**Author:** Mario Zechner (@mariozechner)
**License:** Open source

## What Pi Is

Pi is a modular AI agent framework with 7 npm packages that can be used independently or composed together. It provides a unified LLM API (20+ providers), a stateful agent runtime with tool execution, an interactive terminal coding agent, a TUI framework, a Slack bot, GPU pod management, and web UI components.

## Documentation Goal

Create comprehensive documentation that lets any developer understand:
1. What each package does and why it exists as a separate package
2. How packages depend on each other and communicate
3. The key abstractions (types, classes, interfaces) in each package
4. How data flows through the system during common operations
5. How to extend Pi (extensions, skills, themes, prompts)

## Documentation Structure

```
pi/
├── spec.md                         ← THIS FILE (project tracker)
├── markdown/                       ← Source documentation (viewable on GitHub)
│   ├── README.md                   ← Index / table of contents
│   ├── 00-overview.md              ← What Pi is, philosophy, 7-package map
│   ├── 01-architecture.md          ← Package dependency graph, data flow
│   ├── 02-ai-package.md            ← @mariozechner/pi-ai deep dive
│   ├── 03-agent-core.md            ← @mariozechner/pi-agent-core deep dive
│   ├── 04-coding-agent.md          ← @mariozechner/pi-coding-agent deep dive
│   ├── 05-tui.md                   ← @mariozechner/pi-tui deep dive
│   ├── 06-mom.md                   ← @mariozechner/pi-mom deep dive
│   ├── 07-pods.md                  ← @mariozechner/pi-pods deep dive
│   ├── 08-web-ui.md                ← @mariozechner/pi-web-ui deep dive
│   ├── 09-tool-system.md           ← Cross-cutting: tool definition → execution
│   ├── 10-extension-system.md      ← Cross-cutting: extensions, skills, themes
│   ├── 11-data-flow.md             ← End-to-end flows with sequence diagrams
│   └── 12-sessions.md              ← Pi + Hermes session management deep dive
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
| 1 | Overview: what Pi is, philosophy, package map | DONE | `00-overview.md` |
| 2 | Architecture: dependency graph, communication patterns | DONE | `01-architecture.md` |
| 3 | AI Package: providers, streaming, tools, context | DONE | `02-ai-package.md` |
| 4 | Agent Core: Agent class, event system, tool execution | DONE | `03-agent-core.md` |
| 5 | Coding Agent: CLI, sessions, modes, built-in tools | DONE | `04-coding-agent.md` |
| 6 | TUI: components, rendering, keybindings | DONE | `05-tui.md` |
| 7 | Mom: Slack bot, workspace, sandbox, skills | DONE | `06-mom.md` |
| 8 | Pods: GPU management, vLLM deployment | DONE | `07-pods.md` |
| 9 | Web UI: components, storage, artifacts | DONE | `08-web-ui.md` |
| 10 | Tool System: definition, schema, dispatch, lifecycle | DONE | `09-tool-system.md` |
| 11 | Extension System: extensions, skills, prompts, themes | DONE | `10-extension-system.md` |
| 12 | Data Flow: request lifecycle, streaming, compaction | DONE | `11-data-flow.md` |
| 13 | Sessions: Pi JSONL tree + Hermes SQLite + comparison | DONE | `12-sessions.md` |
| 14 | README index | DONE | `README.md` |

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

**Total Mermaid diagrams rendered:** 33 across all pages (5 in architecture, 1 in ai, 3 in coding-agent, etc.)

### Phase 4: Polish -- DONE

| # | Task | Status | Notes |
|---|------|--------|-------|
| 20 | Cross-reference links between documents | DONE | Relative md links auto-converted to HTML |
| 21 | Code snippet rendering | DONE | Escaped, syntax-tagged, scrollable |
| 22 | Mobile-responsive HTML layout | DONE | Media query at 600px in styles.css |

### Phase 5: Agent Loop Deep Dive

| # | Task | Status | File |
|---|------|--------|------|
| 23 | Agent loop deep dive: loop mechanics, multi-turn, queues, tool pipeline | DONE | `13-agent-loop.md` |

### Phase 6: Model Interop & Memory Expansion

| # | Task | Status | File |
|---|------|--------|------|
| 24 | Model providers: 20+ providers, OpenAI-compatible, Anthropic, local models, model fallbacks | DONE | `14-model-providers.md` |
| 25 | Memory expansion: deep dive into memory types, context management, compaction | DONE | `15-memory-deep.md` |
| 26 | Multi-model execution: background work, parallel model calls, model routing | DONE | `16-multi-model.md` |

### Phase 7: Context Compression Deep Dive

| # | Task | Status | File |
|---|------|--------|------|
| 27 | Context compression: token-budget model, findCutPoint backward walk, split turn parallel summarization, FileOperations tracking, extension hook | DONE | `17-context-compression.md` |

## Build System

**Script:** `documentation/build.py` (shared with Hermes)
**Dependencies:** None (Python 3.12+ stdlib only)
**Features:**
- Converts all markdown to HTML with tables, code blocks, headings, lists, links
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

# Build just Pi
python3 build.py pi

# Build just Hermes
python3 build.py hermes
```

**Rebuild:** Run the same command. It overwrites existing HTML files. Idempotent.

## File Counts

| Type | Count |
|------|-------|
| Markdown source files | 21 |
| Generated HTML files | 22 (21 docs + 1 index) |
| CSS files | 1 (shared) |
| Total HTML output | 14 files + styles.css |

## Expected Outcome

A developer unfamiliar with Pi can:
1. Read the overview and understand what Pi does in 5 minutes
2. Read the architecture doc and understand how the 7 packages fit together
3. Deep-dive into any specific package and understand its internals
4. Follow data flow diagrams to understand what happens during a user interaction
5. Understand how to extend Pi with custom tools, extensions, and skills
6. View the documentation as rendered HTML locally or deploy to GitHub Pages

## Quality Requirements (Iron Rules)

All documentation MUST meet these standards:

1. **Detailed sections with code snippets** — Every concept must be grounded in actual source code. Include real function signatures, class structures, and key logic snippets. No vague hand-waving.
2. **Teach key facts, principles, and ideas quickly** — Each section should deliver insight density. A reader should learn the core concept within the first paragraph, then get progressively deeper detail.
3. **Clear articulation** — Non-overly-complex sentences. Clearly articulated ideas and processes. Every section should flow logically from one idea to the next. Mermaid diagrams and clear logic follow-up for every process.
4. **Mermaid diagrams** — Use mermaid flowcharts, sequence diagrams, and class diagrams to illustrate architecture, data flow, and lifecycle. Minimum 2 diagrams per document.
5. **Good visual assets** — Images, diagrams, tables, ASCII art — anything that helps clearly explain concepts. Diagrams should stand alone as learning aids.
6. **Generated HTML** — All markdown must build to HTML with the shared build.py. Well-aligned headers, text, and menu structure. Modeled after [markdown.engineering/learn-claude-code](https://www.markdown.engineering/learn-claude-code) style: organized, insightful units with clear navigation. See [04-query-engine](https://www.markdown.engineering/learn-claude-code/04-query-engine) for the expected level of header alignment, text quality, and menu structure.
7. **Cross-references** — Every document should link to related documents. No orphan pages.
8. **Source path references** — Include actual file paths from the source codebase so readers can verify claims.

## Grandfather Review

The code is the grandfather — the root of truth. A grandfather review walks back to the source and checks:

1. **Do the names match?** — Types, classes, interfaces, functions, config fields must match the actual TypeScript source. Grep every name in the docs against the codebase.
2. **Do the numbers match?** — Defaults, counts, timeouts, event type totals must match the implementation. If the code emits 20+ event types, the docs must not say "15 events."
3. **Do the flows match?** — State transitions, agent loop steps, request/response shapes must match the actual execution path. Trace the call graph from the entry point.
4. **Is anything missing?** — Features the code has that the docs don't mention. Walk the full public API surface and verify every type, function, and class appears somewhere in the documentation.

**Schedule:** Run a grandfather review after completing all documentation phases and before marking the project as final. Fix every discrepancy — there is no "close enough."

## Resume Point

All phases complete. To continue work:
1. Add new content to `markdown/*.md`, then run `python3 build.py pi` to regenerate HTML
2. Edit `html/styles.css` to adjust styling, then commit
3. For GitHub Pages deployment, push the `html/` directory and configure Pages to serve from it
