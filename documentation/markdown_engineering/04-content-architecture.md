# markdown.engineering -- Content Architecture

## URL Structure and Information Hierarchy

The site organizes ~140 pages into a clear hierarchy that mirrors the six-layer agentic engineering stack.

### Top-Level Navigation

```
/                       Landing page
/stack                  Architecture overview (the six-layer model)
/essays                 Long-form essays (index)
/blog                   Blog posts (index)
/play                   Terminal Quest game (landing)
/learn-claude-code      50-lesson architecture course (landing)
/about                  About the author
```

### Stack Layer Pages

Each layer of the agentic stack gets its own section:

```
/primitives/
  /primitives/markdown
  /primitives/cli
  /primitives/bash
  /primitives/git

/models/
  /models/claude
  /models/gpt
  /models/gemini

/tools/
  /tools/built-in-tools

/mcps/
  /mcps/what-are-mcps
  /mcps/mcp-servers

/skills/
  /skills/what-are-skills

/agents/
  /agents/claude-code
  /agents/codex
  /agents/pi
```

### Content URLs

```
/essays/{slug}                    e.g., /essays/the-markdown-is-the-message
/blog/{YYYY-MM-DD-slug}           e.g., /blog/2026-04-12-agent-engineering-2026
/learn-claude-code/{NN-slug}      e.g., /learn-claude-code/01-boot-sequence
/play/{layer}/{NN}                e.g., /play/primitives/01
```

## Content Collections (Astro)

Astro's content collections system is the backbone of content organization. Each content type lives in a directory under `src/content/` with a schema definition.

### Likely Directory Structure

```
src/
  content/
    config.ts                     # Collection schemas
    essays/
      the-markdown-is-the-message.md
    blog/
      2026-04-12-agent-engineering-2026.md
    stack/
      primitives/
        markdown.md
        cli.md
        bash.md
        git.md
      models/
        claude.md
        gpt.md
        gemini.md
      tools/
        built-in-tools.md
      mcps/
        what-are-mcps.md
        mcp-servers.md
      skills/
        what-are-skills.md
      agents/
        claude-code.md
        codex.md
        pi.md
    lessons/
      01-boot-sequence.md
      02-...md
      ...
      50-architecture-overview.md
    play/
      primitives/
        01.md ... 12.md
      models/
        01.md ... 12.md
      ...
```

### Frontmatter Schema (Inferred)

Each content type uses YAML frontmatter for metadata:

```yaml
# Essays/Blog
---
title: "The Markdown is the Message"
description: "How markdown became the medium of software engineering"
date: 2026-02-23
author: "Noah Green Snow"
tags: ["philosophy", "markdown", "mcluhan"]
og_image: "/images/essays/the-markdown-is-the-message.png"
---

# Lessons
---
title: "Boot Sequence"
lesson: 1
chapter: "Core Architecture"
chapter_tag: "arch"
description: "How Claude Code initializes"
---

# Stack Pages
---
title: "Markdown"
layer: "primitives"
status: "complete"        # or "in-progress"
description: "The foundation of agent communication"
---
```

## Content Status System

Many stack pages use a status badge system:

- **Complete**: Full content, published
- **In progress**: Stub with placeholder text directing readers to @NoahGreenSnow for updates

This is a pragmatic approach -- ship the structure first, fill in content iteratively. The URL structure and navigation are stable even when individual pages are stubs.

## Learn Claude Code Course Structure

The 50-lesson course is organized into 8 chapters, each color-coded:

```
Chapter 1: Core Architecture (arch, orange)
  Lessons covering boot sequence, initialization, configuration

Chapter 2: Tool System (tools, green)
  Lessons covering built-in tools, tool dispatch, permissions

Chapter 3: Agent Intelligence (agents, blue)
  Lessons covering agent loop, decision making, context management

Chapter 4: The Interface (ui, gold)
  Lessons covering Ink/React UI, terminal rendering, user interaction

Chapter 5: Infrastructure (infra, brown)
  Lessons covering session management, storage, caching

Chapter 6: Connectivity (net, teal)
  Lessons covering MCP integration, external services

Chapter 7: Unreleased (leak, red)
  Lessons covering BUDDY companion, ULTRAPLAN, KAIROS, desktop app

Chapter 8: The Big Picture (meta, gray)
  Lessons covering architecture overview, design philosophy
```

Each lesson page includes:
- Phase grids showing key concepts
- Code blocks with syntax highlighting
- Mermaid flowcharts showing system interactions
- Callout boxes (info, tip, warn) for key points
- Details/summary expandable "Deep dive" sections
- Interactive quizzes with answer checking

## Terminal Quest Game Structure

The game maps to the six-layer stack:

```
6 chapters (one per stack layer)
× 12 levels per chapter
= 72 total game levels
```

Each level teaches concepts from its corresponding stack layer through an interactive terminal-based game interface.

## RSS Feed

Available at `/rss.xml`, enabling syndication of blog posts and essays.

## Open Graph / Social Metadata

Every page includes full OG and Twitter Card metadata with custom images per article, ensuring rich previews when shared on social media.

## Key Architectural Takeaways

1. **URL structure mirrors the conceptual model** -- The six-layer stack is reflected directly in the URL hierarchy. Navigation is self-documenting.
2. **Content collections enforce consistency** -- Astro's typed schemas ensure every piece of content has the required metadata.
3. **Progressive content** -- Ship the structure, fill in content over time. Stub pages with status badges set expectations without breaking navigation.
4. **Multiple content formats** -- Essays (deep), blog (timely), lessons (structured), game (interactive) serve different learning styles and entry points.
5. **Flat lesson numbering** -- Lessons use sequential numbers (01-50) rather than nested chapter/lesson numbers. Simple, predictable, easy to reference.
