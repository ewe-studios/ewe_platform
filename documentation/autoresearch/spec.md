# autoresearch Documentation -- Project Spec

## Source Codebase

**Location:** `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AIResearch/autoresearch/`
**Language:** Python 3.10+
**Author:** Andrey Karpathy (@karpathy)
**Date:** March 2026
**License:** MIT
**Based on:** Simplified single-GPU version of [nanochat](https://github.com/karpathy/nanochat)

## What autoresearch Is

autoresearch is an autonomous AI research system where an AI agent experiments with LLM training code overnight. It modifies `train.py`, trains for 5 minutes, checks if `val_bpb` (validation bits per byte) improved, keeps or discards changes, and repeats. Approximately 12 experiments per hour, ~100 per night.

## Documentation Goal

Create comprehensive documentation that lets any developer understand:
1. What autoresearch is and the philosophy behind autonomous AI research
2. How the three key files (`prepare.py`, `train.py`, `program.md`) interact
3. How the autonomous agent loop works (setup, experiment, evaluate, iterate)
4. The model architecture (GPT, value embeddings, sliding window, optimizer)
5. Platform requirements and how to adapt for smaller compute

## Documentation Structure

```
autoresearch/
├── spec.md                         ← THIS FILE (project tracker)
├── markdown/                       ← Source documentation (viewable on GitHub)
│   ├── README.md                   ← Index / table of contents
│   ├── 00-overview.md              ← What autoresearch is, philosophy, vision
│   ├── 01-architecture.md          ← Project structure, 3 key files, data flow
│   ├── 02-agent-research.md        ← Agent loop, val_bpb metric, rules
│   ├── 03-training-setup.md        ← Model architecture, optimizer, training
│   ├── 04-platform-support.md      ← GPU requirements, forks, small compute
│   └── 05-development.md           ← Setup, running, notability
├── html/                           ← Rendered HTML (viewable locally + GitHub Pages)
│   ├── index.html                  ← Auto-generated index + navigation
│   ├── styles.css                  ← Shared CSS (dark/light, responsive)
│   └── 00-overview.html ...        ← Auto-generated from markdown
└── ../build.py                     ← Markdown → HTML (Python stdlib, zero deps)
```

## Tasks

### Phase 1: Core Documentation (Markdown) -- COMPLETE

| # | Task | Status | File |
|---|------|--------|------|
| 1 | README index / catalog | DONE | `markdown/README.md` |
| 2 | Overview: what it is, philosophy, vision from @karpathy | DONE | `00-overview.md` |
| 3 | Architecture: project structure, 3 key files, data flow | DONE | `01-architecture.md` |
| 4 | Agent Research: autonomous experimentation, val_bpb metric | DONE | `02-agent-research.md` |
| 5 | Training Setup: model architecture, optimizer, time budget | DONE | `03-training-setup.md` |
| 6 | Platform Support: GPU requirements, forks, small compute | DONE | `04-platform-support.md` |
| 7 | Development: setup, running the agent, notability | DONE | `05-development.md` |

### Phase 2: HTML Rendering

| # | Task | Status | Notes |
|---|------|--------|-------|
| 8 | Run build.py to generate HTML | TODO | `python3 ../build.py autoresearch` |
| 9 | Verify CSS copied to html/styles.css | TODO | Auto-handled by build.py |
| 10 | Verify Mermaid diagrams render | TODO | 2 diagrams in 01-architecture.md |

### Phase 3: Polish

| # | Task | Status | Notes |
|---|------|--------|-------|
| 11 | Cross-reference links between documents | TODO | Relative md links auto-converted by build.py |
| 12 | Code snippet rendering | TODO | Escaped, syntax-tagged, scrollable |
| 13 | Mobile-responsive HTML layout | TODO | Media query at 600px in styles.css |

## Build System

**Script:** `documentation/build.py` (shared with Pi and Hermes)
**Dependencies:** None (Python 3.12+ stdlib only)
**Features:**
- Converts all markdown to HTML with tables, code blocks, headings, lists, links
- Extracts titles from frontmatter or first `#` heading
- Generates index pages with all document links
- Embeds Mermaid client-side loader (CDN, conditional)
- Embeds dark/light theme toggle with `localStorage` persistence
- Generates prev/next navigation between pages
- Copies shared `styles.css` on first run

**Usage:**
```bash
# Build autoresearch docs
cd documentation && python3 build.py autoresearch

# Build all projects
cd documentation && python3 build.py
```

**Rebuild:** Run the same command. It overwrites existing HTML files. Idempotent.

## File Counts

| Type | Count |
|------|-------|
| Markdown source files | 7 (1 README + 6 docs) |
| Generated HTML files | 8 (7 docs + 1 index) |
| CSS files | 1 (shared) |
| Mermaid diagrams | 2 (in 01-architecture.md) |

## Expected Outcome

A developer unfamiliar with autoresearch can:
1. Read the overview and understand the autonomous research concept in 5 minutes
2. Read the architecture doc and understand how the three files interact
3. Understand how the agent loop works and what the val_bpb metric measures
4. Understand the model architecture and optimizer choices
5. Know how to set up the project and run the agent
6. Know how to adapt the setup for smaller compute platforms
7. View the documentation as rendered HTML locally or deploy to GitHub Pages

## Resume Point

Phase 1 complete. To continue work:
1. Run `python3 ../build.py autoresearch` to generate HTML
2. Verify HTML output and Mermaid diagram rendering
3. For GitHub Pages deployment, push the `html/` directory and configure Pages to serve from it
