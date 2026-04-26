# markdown.engineering -- Technical Site Analysis

## Tech Stack

### Static Site Generator: Astro v5.16.6

The site is built with [Astro](https://astro.build), a modern static site generator. Confirmed via the `<meta name="generator" content="Astro v5.16.6">` tag present on every page.

Astro was chosen because it:
- Outputs fully static HTML with zero JavaScript by default
- Supports Markdown/MDX content collections natively
- Allows scoped component CSS (via `data-astro-cid-*` attributes)
- Can selectively hydrate interactive islands when needed
- Has built-in Shiki code highlighting

### CSS: Tailwind CSS v4.1.18

Tailwind v4 provides the utility layer, confirmed via CSS file headers. However, the site does *not* use Tailwind's default color palette. Instead, it defines a complete custom design token system via CSS custom properties.

### Code Highlighting: Shiki (Astro Built-in)

Code blocks use Astro's built-in Shiki integration with dual-theme support:
- Light theme: `github-light`
- Dark theme: `github-dark`
- Blocks carry the `astro-code` class
- Dual theme switching via `--shiki-light` / `--shiki-dark` CSS variables

The Learn Claude Code lessons additionally use hand-crafted HTML syntax highlighting with semantic CSS classes: `.cm` (comments), `.kw` (keywords), `.fn` (functions), `.str` (strings), `.num` (numbers), `.op` (operators), `.ty` (types).

### Diagrams: Mermaid v11 (CDN)

Mermaid is loaded dynamically from jsDelivr CDN only when `.prose .mermaid` blocks are detected on a page. Key details:

```
CDN: https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs
Security Level: strict
HTML Labels: false
Curve Style: linear
```

Mermaid re-renders on theme toggle via a `MutationObserver` watching the `data-theme` attribute on `<html>`.

### Fonts (Google Fonts)

| Font | Weight | Usage |
|------|--------|-------|
| **Fraunces** (variable, optical size) | 500-700 | Headings (h1, h2, h3) |
| **Manrope** | 400-700 | Body text |
| **IBM Plex Mono** | 400, 500 | Code blocks, labels |
| **Space Grotesk** | 400-600 | Play/Game and Learn pages only |
| **JetBrains Mono** | 400, 500, italic | Play/Game and Learn pages only |

All fonts use `display=swap` for progressive rendering.

### JavaScript

No frontend framework on the main site. Interactive features are implemented with vanilla JavaScript:

- **Theme toggle**: `data-theme` attribute on `<html>`, persisted to `localStorage`, respects `prefers-color-scheme`
- **Scroll animations**: `IntersectionObserver` with staggered delays (70ms per element, max 8), respects `prefers-reduced-motion`
- **Mermaid rendering**: Dynamic ESM import, `MutationObserver` for theme reactivity
- **Quiz components**: Vanilla JS radio button handlers with reveal-on-correct feedback

The Play/Learn section loads a separate JS module for its terminal-based game engine.

## Build Output

Fully static HTML. No client-side hydration islands on the main site pages. CSS is split into:
- One shared global stylesheet
- Per-page scoped stylesheets (Astro's scoped styling)

Theme detection runs synchronously in `<head>` to prevent flash of unstyled content (FOUC).

## Performance Characteristics

- Static HTML = instant TTFB from CDN
- No framework JavaScript on main pages
- Fonts use `display=swap` (no invisible text during load)
- Mermaid loaded only when needed (conditional dynamic import)
- CSS split per-page to avoid loading unused styles
- Animations respect `prefers-reduced-motion`

## What This Means for Replication

The site proves that a documentation-focused site does not need React, Vue, or any SPA framework. The stack is:

1. **Astro** -- static site generator with Markdown content collections
2. **Tailwind** -- utility CSS with custom design tokens
3. **Shiki** -- code highlighting (built into Astro)
4. **Mermaid** -- diagrams loaded from CDN
5. **Vanilla JS** -- theme toggle, scroll animations, quizzes
6. **Google Fonts** -- typography

This is a stack that any team can replicate. The complexity lives in the *content*, not the tooling.
