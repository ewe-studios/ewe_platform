# Documentation

Codebase documentation and reference docs. Organized by project.

## Documented Projects

| Project | What It Is | Docs |
|---------|-----------|------|
| [Pi](./pi/markdown/) | Modular AI agent framework (TypeScript, 7 packages) | [Overview](./pi/markdown/00-overview.md), [Architecture](./pi/markdown/01-architecture.md), [Tool System](./pi/markdown/09-tool-system.md) |
| [Hermes](./hermes/markdown/) | Self-improving AI agent (Python, 40+ tools, 10+ platforms) | [Overview](./hermes/markdown/00-overview.md), [Architecture](./hermes/markdown/01-architecture.md), [Data Flow](./hermes/markdown/11-data-flow.md) |
| [markdown.engineering](./markdown_engineering/) | Site analysis + build-your-own guide | [Overview](./markdown_engineering/00-overview.md), [Build Guide](./markdown_engineering/build-your-own/00-architecture.md) |

## Rendered HTML

Each project has a `html/` directory with browser-ready HTML generated from markdown:

- [Pi HTML](./pi/html/index.html) -- 14 pages with Mermaid diagrams, dark/light theme
- [Hermes HTML](./hermes/html/index.html) -- 14 pages with Mermaid diagrams, dark/light theme

## Build System

A shared Python script converts all markdown to HTML with zero dependencies:

```bash
# Build all projects
python3 build.py

# Build a specific project
python3 build.py pi
python3 build.py hermes
```

The build script:
- Converts markdown to HTML (tables, code blocks, headings, lists, links, blockquotes)
- Embeds Mermaid client-side rendering (CDN, only loads when diagrams exist)
- Embeds dark/light theme toggle with localStorage persistence
- Generates index pages with document navigation
- Generates prev/next navigation between pages
- Uses only Python stdlib -- no pip install needed

## Other Docs

| File | Topic |
|------|-------|
| [aws-lc-sys-linker-fix.md](./aws-lc-sys-linker-fix.md) | AWS-LC linker fix for OpenSSL migration |
| [mtls-fundamentals.md](./mtls-fundamentals.md) | mTLS fundamentals and implementation |
| [fuzz-testing-rust/markdown/00-overview.md](./fuzz-testing-rust/markdown/00-overview.md) | Fuzz testing in Rust — from setup to advanced techniques |
| [property-based-testing-rust/markdown/00-overview.md](./property-based-testing-rust/markdown/00-overview.md) | Property-based testing in Rust — from setup to advanced techniques |
