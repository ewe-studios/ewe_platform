# Graphify Documentation -- Project Spec

## Source Codebase

**Location:** `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.Graphify/graphify/`
**Language:** Python 3.10+
**Version:** 0.5.6
**Author:** Safi Shamsi (@safishamsi)
**License:** Open source
**PyPI:** `graphifyy`

## What Graphify Is

Graphify is an AI coding assistant skill that turns any folder of code, docs, papers, images, or videos into a queryable knowledge graph. It runs a three-pass pipeline: deterministic AST extraction via tree-sitter (25 languages), local audio/video transcription via Whisper, and parallel LLM-driven semantic extraction via Claude subagents. Results are merged into a NetworkX graph, clustered with Leiden community detection, and exported as interactive HTML, queryable JSON, Obsidian vault, and a plain-language audit report. It integrates with 13+ AI coding assistants (Claude Code, Codex, Cursor, Gemini CLI, Aider, Copilot, and more).

## Documentation Goal

Create comprehensive documentation that lets any developer understand:
1. What Graphify does and why it exists — the problem of codebase comprehension at scale
2. The seven-stage pipeline: detect → extract → build → cluster → analyze → report → export
3. How deterministic AST extraction works across 25 languages via tree-sitter
4. How LLM-driven semantic extraction works (Claude subagents, Kimi K2.6 backend)
5. How community detection works (Leiden with Louvain fallback, oversized splitting)
6. How the analysis engine identifies god nodes, surprising connections, and suggested questions
7. How multi-format export works (HTML, JSON, SVG, Obsidian vault, Neo4j, GraphML)
8. How the CLI and multi-platform integration works (13+ coding assistants)
9. How security, caching, and validation work
10. The confidence system (EXTRACTED, INFERRED, AMBIGUOUS) and what each means

## Documentation Structure

```
graphify/
├── spec.md                         ← THIS FILE (project tracker)
├── markdown/                       ← Source documentation (viewable on GitHub)
│   ├── README.md                   ← Index / table of contents
│   ├── 00-overview.md              ← What Graphify is, philosophy, pipeline at a glance
│   ├── 01-architecture.md          ← Module dependency map, pipeline flow, data model
│   ├── 02-detection.md             ← File discovery, classification, corpus health
│   ├── 03-extraction.md            ← Tree-sitter AST extraction, LanguageConfig, 25 languages
│   ├── 04-graph-building.md        ← NetworkX assembly, deduplication, merge strategies
│   ├── 05-clustering.md            ← Leiden/Louvain community detection, splitting, cohesion
│   ├── 06-analysis.md              ← God nodes, surprising connections, suggested questions
│   ├── 07-report-export.md         ← GRAPH_REPORT.md generation, HTML/JSON/SVG/Obsidian/Neo4j export
│   ├── 08-cli-integration.md       ← CLI commands, 13+ platform install, hooks, skill files
│   ├── 09-security-validation.md   ← SSRF guards, path traversal, label sanitization, schema validation
│   ├── 10-caching-performance.md   ← SHA256 caching, incremental updates, token benchmarking
│   ├── 11-data-flow.md             ← End-to-end flows with sequence diagrams
│   └── 12-llm-backend.md          ← Direct LLM extraction (Claude API, Kimi K2.6), transcription
├── html/                           ← Rendered HTML (viewable locally + GitHub Pages)
│   ├── index.html                  ← Auto-generated index + navigation
│   ├── styles.css                  ← Shared CSS (dark/light, responsive)
│   └── 00-overview.html ...        ← Auto-generated from markdown
└── build.py                        ← Markdown → HTML (Python stdlib, zero deps)
```

## Tasks

### Phase 1: Core Documentation (Markdown)

| # | Task | Status | File |
|---|------|--------|------|
| 1 | Overview: what Graphify is, philosophy, pipeline map | DONE | `00-overview.md` |
| 2 | Architecture: module map, pipeline flow, data model | DONE | `01-architecture.md` |
| 3 | Detection: file discovery, FileType enum, corpus health | DONE | `02-detection.md` |
| 4 | Extraction: tree-sitter, LanguageConfig, 25 languages, caching | DONE | `03-extraction.md` |
| 5 | Graph Building: NetworkX assembly, deduplication, merge | DONE | `04-graph-building.md` |
| 6 | Clustering: Leiden/Louvain, splitting, cohesion scoring | DONE | `05-clustering.md` |
| 7 | Analysis: god nodes, surprising connections, questions | DONE | `06-analysis.md` |
| 8 | Report & Export: GRAPH_REPORT.md, HTML, JSON, SVG, Obsidian, Neo4j | DONE | `07-report-export.md` |
| 9 | CLI & Integration: commands, 13+ platforms, hooks, skill files | DONE | `08-cli-integration.md` |
| 10 | Security & Validation: SSRF, path traversal, sanitization, schema | DONE | `09-security-validation.md` |
| 11 | Caching & Performance: SHA256, incremental, benchmarking | DONE | `10-caching-performance.md` |
| 12 | Data Flow: end-to-end flows with sequence diagrams | DONE | `11-data-flow.md` |
| 13 | LLM Backend: direct API, Kimi, transcription | DONE | `12-llm-backend.md` |
| 14 | README index | DONE | `README.md` |

### Phase 2: HTML Rendering

| # | Task | Status | File |
|---|------|--------|------|
| 15 | Shared CSS (dark/light, code blocks, diagrams) | TODO | `html/styles.css` |
| 16 | Index page with navigation | TODO | `html/index.html` |
| 17 | Generate all HTML pages | TODO | `html/*.html` |

### Phase 3: Diagrams

| # | Task | Status | Notes |
|---|------|--------|-------|
| 18 | Mermaid rendering in HTML pages | TODO | Client-side via CDN |
| 19 | Theme-aware diagram re-rendering | TODO | Re-renders on dark/light toggle |

### Phase 4: Polish

| # | Task | Status | Notes |
|---|------|--------|-------|
| 20 | Cross-reference links between documents | TODO | Relative md links auto-converted |
| 21 | Code snippet rendering | TODO | Escaped, syntax-tagged, scrollable |
| 22 | Mobile-responsive HTML layout | TODO | Media query at 600px |

## Build System

**Script:** `documentation/build.py` (shared with Pi, Hermes, Mastra)
**Dependencies:** None (Python 3.12+ stdlib only)

**Usage:**
```bash
cd documentation && python3 build.py graphify
```

## Quality Requirements (Iron Rules)

All documentation MUST meet these standards:

1. **Detailed sections with code snippets** — Every concept must be grounded in actual source code. Include real function signatures, class structures, and key logic snippets. No vague hand-waving.
2. **Teach key facts, principles, and ideas quickly** — Each section should deliver insight density. A reader should learn the core concept within the first paragraph, then get progressively deeper detail.
3. **Clear articulation** — Non-overly-complex sentences. Clearly articulated ideas and processes. Every section should flow logically from one idea to the next.
4. **Mermaid diagrams** — Use mermaid flowcharts, sequence diagrams, and class diagrams to illustrate architecture, data flow, and lifecycle. Minimum 2 diagrams per document.
5. **Good visual assets** — Tables for comparisons, ASCII art for quick structure overviews, mermaid for complex flows. Diagrams should stand alone as learning aids.
6. **Generated HTML** — All markdown must build to HTML with the shared build.py. Well-aligned headers, text, and menu structure. Modeled after markdown.engineering/learn-claude-code style: organized, insightful units with clear navigation.
7. **Cross-references** — Every document should link to related documents. No orphan pages.
8. **Source path references** — Include actual file paths from the source codebase so readers can verify claims.

## Expected Outcome

A developer unfamiliar with Graphify can:
1. Read the overview and understand what Graphify does in 5 minutes
2. Read the architecture doc and understand the 7-stage pipeline
3. Deep-dive into any pipeline stage (detection, extraction, clustering, etc.)
4. Understand the confidence system (EXTRACTED, INFERRED, AMBIGUOUS)
5. Install Graphify on their preferred AI coding assistant
6. Query the knowledge graph via CLI, MCP server, or JSON
7. View the documentation as rendered HTML locally or deploy to GitHub Pages

## Resume Point

Phase 1 complete. To continue work:
1. Run `python3 build.py graphify` to generate HTML from markdown
2. Verify HTML rendering, mermaid diagrams, navigation
3. Run grandfather review against source code
