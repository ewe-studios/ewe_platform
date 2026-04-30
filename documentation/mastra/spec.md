# Mastra Documentation -- Project Spec

## Source Codebase

**Location:** `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.mastra-ai/mastra/`
**Language:** TypeScript (monorepo, pnpm workspaces)
**Version:** Latest (active development)
**Author:** Mastra AI
**License:** MIT

## What Mastra Is

Mastra is a TypeScript-first AI agent framework that provides a unified model router (200+ providers via OpenAI-compatible interface), a workflow-based agentic loop with tool execution, built-in memory with semantic recall and observational memory, background task execution, and a processor pipeline system for input/output/error handling. It supports multi-model fallbacks, tool suspension/resumption, human-in-the-loop approval, and sub-agent delegation.

## Documentation Goal

Create comprehensive documentation that lets any developer understand:
1. How the agent class orchestrates LLM interactions, memory, tools, and processors
2. How the workflow-based agentic loop works (the core while-loop architecture)
3. How tool calling, suspension, approval, and background execution work
4. How the model router resolves provider configurations (OpenAI, Anthropic, local models, etc.)
5. How memory works (semantic recall, working memory, observational memory)
6. How the processor pipeline transforms inputs/outputs/errors
7. How multi-model execution and fallback chains operate
8. How background tasks enable async tool execution

## Documentation Structure

```
mastra/
├── spec.md                         ← THIS FILE (project tracker)
├── markdown/                       ← Source documentation (viewable on GitHub)
│   ├── README.md                   ← Index / table of contents
│   ├── 00-overview.md              ← What Mastra is, philosophy, architecture
│   ├── 01-architecture.md          ← Package map, dependency graph, layers
│   ├── 02-agent-core.md            ← Agent class, generate/stream, execution
│   ├── 03-agent-loop.md            ← Workflow-based agentic loop deep dive
│   ├── 04-tool-system.md           ← Tool calling, suspension, approval, background
│   ├── 05-model-router.md          ← Provider resolution, OpenAI-compatible, Anthropic, local
│   ├── 06-memory-system.md         ← Memory, semantic recall, working memory, OM
│   ├── 07-processors.md            ← Input/output/error processor pipeline
│   ├── 08-multi-model.md           ← Model fallbacks, background tasks, delegation
│   ├── 09-data-flow.md             ← End-to-end flows with sequence diagrams
│   ├── 10-comparison.md            ← Mastra vs Pi vs Hermes comparison
│   ├── 11-context-compression.md   ← Context management, memory-driven selection, no post-hoc compression
│   ├── 12-async-typescript.md      ← TransformStream, Promise.allSettled, pubsub, LLM recorder
│   ├── 13-multi-model-deep.md      ← Model fallbacks, credential management, error classification
│   └── 14-rl-training-traces.md    ← Observability spans, LLM recording, training data comparison
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
| 1 | Overview: what Mastra is, philosophy, architecture | TODO | `00-overview.md` |
| 2 | Architecture: package map, dependency graph, layers | TODO | `01-architecture.md` |
| 3 | Agent Core: Agent class, generate/stream, execution | TODO | `02-agent-core.md` |
| 4 | Agent Loop: workflow-based agentic loop deep dive | TODO | `03-agent-loop.md` |
| 5 | Tool System: tool calling, suspension, approval, background | TODO | `04-tool-system.md` |
| 6 | Model Router: provider resolution, OpenAI-compatible, Anthropic, local | TODO | `05-model-router.md` |
| 7 | Memory System: semantic recall, working memory, observational memory | TODO | `06-memory-system.md` |
| 8 | Processors: input/output/error processor pipeline | TODO | `07-processors.md` |
| 9 | Multi-Model: fallbacks, background tasks, delegation | TODO | `08-multi-model.md` |
| 10 | Data Flow: end-to-end flows with sequence diagrams | TODO | `09-data-flow.md` |
| 11 | Comparison: Mastra vs Pi vs Hermes | TODO | `10-comparison.md` |
| 12 | README index | TODO | `README.md` |

### Phase 2: HTML Rendering

| # | Task | Status | File |
|---|------|--------|------|
| 13 | Shared CSS (dark/light, code blocks, diagrams) | TODO | `html/styles.css` |
| 14 | Index page with navigation | TODO | `html/index.html` |
| 15 | Generate all HTML pages | TODO | `html/*.html` |

### Phase 3: Diagrams

| # | Task | Status | Notes |
|---|------|--------|-------|
| 16 | Mermaid rendering in HTML pages | TODO | Client-side via CDN |
| 17 | Theme-aware diagram re-rendering | TODO | Re-renders on dark/light toggle |

### Phase 4: Polish

| # | Task | Status | Notes |
|---|------|--------|-------|
| 18 | Cross-reference links between documents | TODO | Relative md links auto-converted |
| 19 | Code snippet rendering | TODO | Escaped, syntax-tagged, scrollable |
| 20 | Mobile-responsive HTML layout | TODO | Media query at 600px |

### Phase 6: Ecosystem and Plugin Coverage

| # | Task | Status | File |
|---|------|--------|------|
| 25 | Ecosystem: production services, applications, workshops, templates, and all 27 directories outside mastra/core | DONE | `15-ecosystem.md` |
| 26 | Plugin ecosystem: auth, browser, client-sdks, deployers, observability, stores, voice, server-adapters, workflows, pubsub, workspaces, integrations, mastracode | DONE | `16-plugin-ecosystem.md` |
| 27 | Examples: agent/ (primitives, elicitation, presets, trace seeding) and agent-v6/ (structured output) | DONE | `17-examples.md` |
| 28 | Official docs/: Docusaurus site, 5 collections, 18 doc sections, 13 tutorials, migrations | DONE | `18-docs.md` |

### Phase 5: Deep Dives (Compression, Async, Multi-Model, RL)

| # | Task | Status | File |
|---|------|--------|------|
| 21 | Context compression: Mastra uses memory-driven selection (semantic recall + working memory) instead of post-hoc compression. Compare Pi's token reserve vs Hermes's percentage trigger vs Mastra's proactive approach | DONE | `11-context-compression.md` |
| 22 | Async/TypeScript patterns: TransformStream streaming, Promise.allSettled tool isolation, pubsub background tasks, MSW LLM recorder/replay. Compare Hermes's sync-first + asyncio bridges vs Pi's async run() vs Mastra's workflow async | DONE | `12-async-typescript.md` |
| 23 | Multi-model execution: ModelRouterLanguageModel provider resolution, fallback chains, error classification, usage metrics extraction. Compare Hermes's credential pool + NousRateGuard vs Pi's provider adapters vs Mastra's gateway plugins | DONE | `13-multi-model-deep.md` |
| 24 | Observability/traces: OpenTelemetry span hierarchy (MODEL_GENERATION → MODEL_STEP → MODEL_CHUNK), LLM recorder with MSW interception, content summarization for bounded spans. Compare Hermes's ShareGPT trajectory recording vs Mastra's observability-first approach | DONE | `14-rl-training-traces.md` |

## Build System

**Script:** `documentation/build.py` (shared with Pi and Hermes)
**Dependencies:** None (Python 3.12+ stdlib only)

**Usage:**
```bash
cd documentation && python3 build.py mastra
```

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

1. **Do the names match?** — Classes, methods, types, interfaces, config fields must match the actual TypeScript source. Grep every name in the docs against the codebase.
2. **Do the numbers match?** — Defaults, provider counts (200+), memory limits, timeout values must match the implementation.
3. **Do the flows match?** — Workflow-based agent loop, tool suspension/resumption, processor pipeline, model router resolution must match the actual execution path.
4. **Is anything missing?** — Features the code has that the docs don't mention. Walk every module, every public class, and verify each appears somewhere in the documentation.

**Schedule:** Run a grandfather review after completing all documentation phases and before marking the project as final. Fix every discrepancy — there is no "close enough."

## Expected Outcome

A developer unfamiliar with Mastra can:
1. Read the overview and understand what Mastra does in 5 minutes
2. Read the architecture doc and understand how the packages fit together
3. Deep-dive into the agent loop and understand the workflow-based execution
4. Understand how model routing, tool calling, memory, and processors work
5. Compare Mastra's approach with Pi and Hermes
