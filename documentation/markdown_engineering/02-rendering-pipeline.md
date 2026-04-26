# markdown.engineering -- Rendering Pipeline

## From Markdown to Browser

The rendering pipeline transforms authored Markdown files into themed, interactive HTML pages with diagrams, code highlighting, and animations. Here is the complete flow.

## Pipeline Stages

```
Stage 1: Authoring
  Markdown files with YAML frontmatter
  ├── Content: prose, headings, lists, tables
  ├── Code blocks: fenced with language tags
  ├── Mermaid blocks: fenced as ```mermaid
  └── Custom components: callouts, quizzes, phase grids

Stage 2: Astro Content Collections
  Astro reads the content/ directory
  ├── Parses YAML frontmatter (title, date, description, tags)
  ├── Validates schema against collection definitions
  └── Exposes typed data to page templates

Stage 3: Markdown Processing (at build time)
  Astro's built-in markdown pipeline
  ├── Converts markdown to HTML via remark/rehype
  ├── Shiki processes code blocks → syntax-highlighted HTML
  │   ├── Assigns dual-theme CSS variables (--shiki-light, --shiki-dark)
  │   └── Wraps in <pre class="astro-code"> elements
  ├── Mermaid blocks are NOT processed here
  │   └── They pass through as <div class="mermaid"> or <pre class="mermaid">
  └── Custom remark/rehype plugins handle extensions (callouts, etc.)

Stage 4: Astro Page Generation (at build time)
  .astro page templates receive processed content
  ├── Layout components wrap content (header, nav, footer)
  ├── Scoped CSS is generated (data-astro-cid-* attributes)
  ├── Conditional scripts are injected:
  │   ├── Theme toggle script (always, in <head>)
  │   ├── Scroll reveal script (pages with .reveal elements)
  │   └── Mermaid loader script (pages with .prose .mermaid blocks)
  └── Static HTML files are written to dist/

Stage 5: Client-Side Rendering (in browser)
  Browser loads static HTML
  ├── Theme script runs synchronously (prevents FOUC)
  ├── CSS renders with correct theme variables
  ├── Fonts load progressively (display=swap)
  ├── IntersectionObserver triggers scroll animations
  └── Mermaid.js (if present):
      ├── Dynamic ESM import from CDN
      ├── Reads CSS custom properties for theme colors
      ├── Renders .mermaid blocks into SVG
      └── MutationObserver watches data-theme for re-renders
```

## Detailed: Mermaid Rendering Flow

The Mermaid integration is the most interesting part of the pipeline because it bridges build-time and client-time processing.

### Build Time

Mermaid code blocks in Markdown are NOT rendered at build time. They pass through Astro's markdown pipeline as raw `<div class="mermaid">` elements containing the Mermaid source text. This is intentional -- Mermaid requires a DOM to render into SVG.

### Client Time

```javascript
// Pseudocode of the Mermaid loader (reconstructed from site behavior)

// 1. Check if page has mermaid blocks
const hasMermaid = document.querySelector('.prose .mermaid');
if (!hasMermaid) return;  // Don't load Mermaid at all

// 2. Dynamic import from CDN
const mermaid = await import(
  'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs'
);

// 3. Read current theme's CSS custom properties
const style = getComputedStyle(document.documentElement);
const themeVars = {
  primaryColor:     style.getPropertyValue('--bg-strong'),
  primaryTextColor: style.getPropertyValue('--fg'),
  lineColor:        style.getPropertyValue('--line-strong'),
  // ... maps all site tokens to Mermaid theme variables
};

// 4. Initialize with strict security and theme mapping
mermaid.default.initialize({
  startOnLoad: false,
  securityLevel: 'strict',
  htmlLabels: false,
  flowchart: { curve: 'linear' },
  themeVariables: themeVars,
});

// 5. Render all mermaid blocks
await mermaid.default.run({ nodes: document.querySelectorAll('.prose .mermaid') });

// 6. Watch for theme changes and re-render
const observer = new MutationObserver(async (mutations) => {
  for (const mutation of mutations) {
    if (mutation.attributeName === 'data-theme') {
      // Re-read CSS properties, re-initialize, re-render
      await reRenderMermaid();
    }
  }
});
observer.observe(document.documentElement, { attributes: true });
```

### Why Client-Side Rendering?

1. **Theme reactivity** -- Mermaid diagrams must re-render when the user toggles dark/light mode. Pre-rendered SVGs cannot change colors.
2. **DOM dependency** -- Mermaid needs a real DOM to calculate layout and produce SVG paths.
3. **CDN efficiency** -- Loading Mermaid from jsDelivr means the site's own static bundle stays small. Users who hit CDN-cached Mermaid pay near-zero cost.
4. **Conditional loading** -- Pages without diagrams never load the Mermaid library at all.

## Detailed: Code Highlighting Flow

### Standard Code Blocks (Shiki)

```
Markdown:        ```rust
                 fn main() { println!("hello"); }
                 ```

Shiki processes:  Tokenizes using TextMate grammar for Rust
                  Assigns dual-theme colors as CSS variables

HTML output:     <pre class="astro-code" style="--shiki-light:#24292e;--shiki-dark:#e1e4e8"
                   data-language="rust">
                   <code>
                     <span style="--shiki-light:#d73a49;--shiki-dark:#f97583">fn</span>
                     <span style="--shiki-light:#6f42c1;--shiki-dark:#b392f0"> main</span>
                     ...
                   </code>
                 </pre>

CSS switches:    [data-theme="light"] .astro-code span { color: var(--shiki-light); }
                 [data-theme="dark"]  .astro-code span { color: var(--shiki-dark); }
```

### Custom Code Blocks (Learn Lessons)

The Learn Claude Code lessons use hand-crafted HTML instead of Shiki, with semantic CSS classes:

```html
<pre class="code-block">
  <code>
    <span class="kw">async</span> <span class="kw">function</span>
    <span class="fn">processTools</span>(<span class="ty">messages</span>) {
      <span class="cm">// Tool dispatch loop</span>
      <span class="kw">for</span> (<span class="kw">const</span> msg <span class="kw">of</span> messages) {
        <span class="kw">await</span> <span class="fn">dispatch</span>(msg);
      }
    }
  </code>
</pre>
```

This approach gives full control over highlighting semantics but requires manual markup.

## Detailed: Theme System Flow

```
User toggles theme
  → JavaScript sets data-theme="dark" on <html>
  → localStorage.setItem('theme', 'dark')
  → CSS custom properties cascade:
      [data-theme="dark"] {
        --bg: #13100f;
        --fg: #f7efe4;
        --accent: #d4a176;
        ...
      }
  → All CSS-variable-based colors update instantly
  → Shiki code blocks swap via var(--shiki-dark)
  → MutationObserver fires
  → Mermaid re-reads CSS properties and re-renders SVGs
```

## Key Architectural Decisions

1. **Static-first**: Everything that can be computed at build time is. Only theme-reactive elements (Mermaid, Shiki theme swap) need client JS.
2. **No hydration**: No React/Vue/Svelte islands. The interactive elements (theme toggle, scroll reveal, quiz) are small enough for vanilla JS.
3. **Conditional loading**: Mermaid JS is only loaded on pages that need it. No global bundle bloat.
4. **CSS custom properties as the integration layer**: Both Shiki and Mermaid read CSS variables for theming, so the single source of truth is the CSS token system.
