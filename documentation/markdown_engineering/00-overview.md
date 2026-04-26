# markdown.engineering -- Site Overview

## What It Is

**markdown.engineering** is a documentation-as-philosophy site created by Noah Green Snow. It coins and explores the discipline of "Markdown Engineering" -- the practice of building and maintaining the markdown systems that control how AI agents write software.

The site's tagline: **"The code is generated. The markdown is engineered."**

## Core Thesis

In the age of AI coding agents (Claude Code, Codex, Gemini CLI), the actual engineering work has shifted. Developers no longer write most code by hand. Instead, they write the markdown files -- `CLAUDE.md`, skill definitions, agent rules, test specifications -- that instruct and constrain those agents. The markdown *is* the engineering artifact.

The site draws on Marshall McLuhan's media theory ("The Medium is the Message") to argue that markdown has become the medium reshaping software development. While everyone focuses on the generated code (the content), the markdown files that produce that code are the real product.

## Key Distinction: Markdown Engineering vs Prompt Engineering

| Aspect | Prompt Engineering | Markdown Engineering |
|--------|-------------------|---------------------|
| Persistence | Disposable, session-bound | Versioned, lives in git |
| Composability | One-shot strings | Modular, composable files |
| Tracking | No history | Full git history |
| Scope | Single interaction | Entire project lifecycle |
| Artifact | The prompt text | The file system structure |

## The Agentic Engineering Stack

The site defines a six-layer stack for understanding how AI agents interact with codebases:

```
Layer 6: Agents/Harness   -- Claude Code, Codex, Pi (the runtime)
Layer 5: Skills            -- Workflows, slash commands, SKILL.md files
Layer 4: MCPs              -- Model Context Protocol servers, integrations
Layer 3: Tools             -- Bash, Read, Edit, Write, Grep, Glob
Layer 2: Models            -- Claude, GPT, Gemini (the LLMs themselves)
Layer 1: Primitives        -- Markdown, CLI, Bash, Git
```

Each layer builds on the one below. The site has dedicated sections exploring each layer.

## Site Author

Noah Green Snow (@NoahGreenSnow on X/Twitter). No public GitHub repository for the site source exists. The site itself is the artifact.

## Published Content

The site contains three major content types:

1. **Essays** -- Long-form pieces like "The Markdown is the Message" (Feb 23, 2026)
2. **Blog posts** -- Shorter pieces like "Agent Engineering in 2026: The Harness Is the Product" (Apr 12, 2026)
3. **Learn Claude Code** -- A 50-lesson architecture deep-dive course built from Claude Code's source
4. **Terminal Quest** -- An interactive terminal-based learning game with 72 levels across 6 chapters
5. **Stack pages** -- Reference documentation for each layer of the stack

Many concept pages are stubs marked "In progress." The substantial content is concentrated in the essay, blog, and Learn Claude Code sections.

## Why This Site Matters

It represents a new category of technical documentation -- documentation about *how to document for AI agents*. The site itself demonstrates the principles it teaches: structured markdown, clear hierarchy, composable content, and machine-readable organization.
