# markdown.engineering -- Documentation & Build Guide

## Site Analysis

Detailed analysis of [markdown.engineering](https://www.markdown.engineering), a documentation site that coins the discipline of "Markdown Engineering" -- building the markdown systems that control how AI agents write software.

| Document | Covers |
|----------|--------|
| [00-overview.md](./00-overview.md) | What the site is, its philosophy, the agentic engineering stack |
| [01-site-analysis.md](./01-site-analysis.md) | Full tech stack: Astro, Tailwind, Shiki, Mermaid, fonts, JS |
| [02-rendering-pipeline.md](./02-rendering-pipeline.md) | How markdown becomes themed HTML with diagrams and code blocks |
| [03-design-system.md](./03-design-system.md) | Color tokens, typography, component patterns, responsive design |
| [04-content-architecture.md](./04-content-architecture.md) | URL structure, content collections, frontmatter schemas |
| [05-features-deep-dive.md](./05-features-deep-dive.md) | Every interactive feature: theme toggle, Mermaid, quizzes, scroll reveal |

## Build Your Own

Step-by-step guide to building a similar documentation site powered by LLM-generated content, Mermaid diagrams, and markdown.

| Document | Covers |
|----------|--------|
| [00-architecture.md](./build-your-own/00-architecture.md) | Four-layer architecture: Analysis → LLM → Content → Rendering |
| [01-project-setup.md](./build-your-own/01-project-setup.md) | Astro project, Tailwind, content collections, base layout, CSS tokens |
| [02-markdown-pipeline.md](./build-your-own/02-markdown-pipeline.md) | Markdown file structure, frontmatter schemas, callouts, source links |
| [03-mermaid-integration.md](./build-your-own/03-mermaid-integration.md) | Client-side Mermaid rendering, theme-aware config, remark plugin |
| [04-code-snippets.md](./build-your-own/04-code-snippets.md) | Symbol extraction, snippet selection, source reference links |
| [05-llm-powered-insights.md](./build-your-own/05-llm-powered-insights.md) | LLM generation script, prompt templates, caching, batching, costs |
| [06-connecting-the-dots.md](./build-your-own/06-connecting-the-dots.md) | Dependency extraction, cross-references, navigation, search |
| [07-deployment.md](./build-your-own/07-deployment.md) | Hosting, CI/CD pipelines, incremental regeneration, production checklist |

## Architecture Summary

```
YOUR CODEBASE
     │
     ▼
 Analysis Scripts ──→ JSON (structure, deps, symbols)
     │
     ▼
 LLM Generation   ──→ Markdown files (prose + code + diagrams)
     │
     ▼
 Astro + Shiki    ──→ Static HTML (themed, highlighted, interactive)
     │
     ▼
 Static Host      ──→ Your documentation site
```

Four layers. Files as interfaces. No databases. No custom servers. Under 1,000 lines of custom code.
