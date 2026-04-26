# Build Your Own -- Architecture Overview

## Goal

Build a documentation site that, given a codebase, produces:
- Markdown pages with prose explanations
- Code snippets pulled from actual source files
- Mermaid diagrams showing how components connect
- Cross-references linking concepts to their implementations

The LLM generates the content. The tooling renders it. The human curates and publishes.

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      YOUR CODEBASE                          │
│  src/ lib/ tests/ configs/ ...                              │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                   ANALYSIS LAYER                            │
│                                                             │
│  1. File Discovery     find/glob all source files           │
│  2. Dependency Graph   imports, calls, module boundaries    │
│  3. Symbol Extraction  functions, types, exports            │
│  4. Test Mapping       which tests cover which modules      │
│  5. Git Context        recent changes, blame, contributors  │
│                                                             │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                    LLM LAYER                                │
│                                                             │
│  Input:  structured context (file content, dependency       │
│          graph, symbols, tests, git history)                │
│                                                             │
│  Output: for each module/component:                         │
│          - prose explanation (markdown)                      │
│          - code snippet selections (with line references)   │
│          - mermaid diagram source (flowchart/sequence)       │
│          - cross-references to related modules              │
│                                                             │
│  Model:  Claude, GPT-4, or any capable LLM                 │
│  API:    Anthropic SDK / OpenAI SDK                         │
│                                                             │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                  CONTENT LAYER                              │
│                                                             │
│  Generated Markdown files with frontmatter:                 │
│  content/                                                   │
│    modules/                                                 │
│      auth.md          ← explanation + snippets + diagrams   │
│      database.md                                            │
│      api-routes.md                                          │
│    connections/                                              │
│      auth-to-database.md  ← how auth uses the DB layer     │
│    overview.md            ← high-level architecture         │
│                                                             │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                  RENDERING LAYER                            │
│                                                             │
│  Astro (static site generator)                              │
│  ├── Reads content/ as content collections                  │
│  ├── Shiki highlights code blocks at build time             │
│  ├── Mermaid renders diagrams at client time                │
│  ├── Tailwind + CSS custom properties for theming           │
│  └── Outputs static HTML to dist/                           │
│                                                             │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                  HOSTING                                     │
│                                                             │
│  Any static host: Vercel, Netlify, Cloudflare Pages,        │
│  GitHub Pages, S3+CloudFront                                │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## The Four Layers Explained

### 1. Analysis Layer (Scripts You Write)

A set of scripts that extract structured information from your codebase. These run locally and produce JSON files that the LLM layer consumes.

**Tools:** Node.js/Python scripts, `tree-sitter` for parsing, `madge` for JS dependency graphs, `cargo metadata` for Rust, `find`/`grep`/`jq` for everything else.

**Output:** JSON files describing your codebase's structure, dependencies, and content.

### 2. LLM Layer (API Calls)

A script that feeds the analysis output to an LLM and asks it to generate documentation. The LLM receives structured context (not raw file dumps) and produces structured output (markdown with frontmatter, mermaid source, snippet references).

**Tools:** Anthropic SDK or OpenAI SDK. A single script with well-crafted prompts.

**Output:** Markdown files ready for the content layer.

### 3. Content Layer (Markdown Files)

The generated markdown files live in a `content/` directory. Each file has YAML frontmatter for metadata and contains prose, code blocks, and mermaid diagram definitions.

**Tools:** Just the filesystem. Files are markdown with frontmatter.

**Output:** A directory of `.md` files.

### 4. Rendering Layer (Static Site Generator)

Astro reads the content directory, processes markdown through Shiki for code highlighting, and outputs static HTML. Mermaid diagrams render client-side. A small CSS design system handles theming.

**Tools:** Astro, Tailwind CSS, Mermaid (CDN), Google Fonts.

**Output:** Static HTML site.

## What Makes This Architecture Simple

1. **Each layer has one job.** Analysis extracts. LLM generates. Content stores. Rendering displays.
2. **Layers communicate via files.** Analysis produces JSON. LLM produces Markdown. Rendering reads Markdown. No databases, no APIs between layers, no message queues.
3. **Each layer is independently replaceable.** Swap Astro for Hugo. Swap Claude for GPT-4. Swap the analysis scripts for a different language. The interfaces (JSON files, Markdown files) stay the same.
4. **The LLM is a batch process, not a runtime dependency.** Generate documentation once, rebuild when the codebase changes. The rendered site has zero LLM dependency.
5. **No custom servers.** Everything is static files or CLI scripts.

## Directory Structure

```
your-docs-site/
├── analyze/                    # Analysis layer scripts
│   ├── extract-structure.ts    # File/module discovery
│   ├── extract-deps.ts         # Dependency graph
│   ├── extract-symbols.ts      # Function/type extraction
│   └── output/                 # Generated JSON
│       ├── structure.json
│       ├── dependencies.json
│       └── symbols.json
├── generate/                   # LLM layer scripts
│   ├── generate-docs.ts        # Main generation script
│   ├── prompts/                # Prompt templates
│   │   ├── module-doc.md       # Prompt for module documentation
│   │   ├── connection-doc.md   # Prompt for connection documentation
│   │   └── overview-doc.md     # Prompt for architecture overview
│   └── config.ts               # LLM API configuration
├── site/                       # Rendering layer (Astro project)
│   ├── src/
│   │   ├── content/            # Generated markdown (from LLM layer)
│   │   │   ├── config.ts       # Collection schemas
│   │   │   ├── modules/        # Per-module documentation
│   │   │   ├── connections/    # Cross-module documentation
│   │   │   └── overview.md     # Architecture overview
│   │   ├── layouts/            # Page layouts
│   │   ├── components/         # Reusable UI components
│   │   ├── pages/              # Route definitions
│   │   └── styles/             # CSS design tokens
│   ├── public/                 # Static assets
│   ├── astro.config.mjs
│   └── package.json
├── scripts/
│   ├── full-pipeline.sh        # Run analyze → generate → build
│   └── watch.sh                # Re-run on codebase changes
└── README.md
```

## Pipeline Execution

```bash
# Full pipeline: analyze codebase, generate docs, build site
./scripts/full-pipeline.sh /path/to/your/codebase

# Which runs:
# 1. node analyze/extract-structure.ts /path/to/your/codebase
# 2. node analyze/extract-deps.ts /path/to/your/codebase
# 3. node analyze/extract-symbols.ts /path/to/your/codebase
# 4. node generate/generate-docs.ts
# 5. cd site && npm run build
```

The pipeline is idempotent. Running it again overwrites previous output. Git-track the generated markdown if you want to review LLM output diffs.

## Cost Estimate

For a medium codebase (~200 files, ~50 modules):
- Analysis: seconds (local scripts)
- LLM generation: ~50 API calls, ~$2-5 with Claude Sonnet
- Build: seconds (Astro static build)
- Total: under 5 minutes, under $5
