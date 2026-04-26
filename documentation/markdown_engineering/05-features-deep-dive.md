# markdown.engineering -- Features Deep Dive

## Interactive Features Analysis

This document examines every interactive and dynamic feature on the site, how each is implemented, and what makes it work.

---

## 1. Theme System (Dark/Light Mode)

### What Users See
A toggle button in the header switches between light and dark themes. The transition is instant across all elements including code blocks and diagrams.

### How It Works

**Initialization (synchronous, in `<head>`):**
```html
<script>
  // Runs before any content paints -- prevents flash of wrong theme
  (function() {
    const saved = localStorage.getItem('theme');
    const preferred = window.matchMedia('(prefers-color-scheme: dark)').matches
      ? 'dark' : 'light';
    document.documentElement.setAttribute('data-theme', saved || preferred);
  })();
</script>
```

**Toggle handler:**
```javascript
function toggleTheme() {
  const current = document.documentElement.getAttribute('data-theme');
  const next = current === 'dark' ? 'light' : 'dark';
  document.documentElement.setAttribute('data-theme', next);
  localStorage.setItem('theme', next);
}
```

**CSS cascades from there:**
```css
[data-theme="light"] { --bg: #f8f1e6; --fg: #2b2018; /* ... */ }
[data-theme="dark"]  { --bg: #13100f; --fg: #f7efe4; /* ... */ }

body { background: var(--bg); color: var(--fg); }
```

**Why this approach:**
- `data-theme` attribute on `<html>` is the single source of truth
- CSS custom properties cascade automatically -- no JS needed to update individual elements
- Mermaid watches `data-theme` via MutationObserver to re-render
- Shiki uses CSS variable selectors to swap code colors
- Zero layout shift because the script runs before first paint

---

## 2. Mermaid Diagram Integration

### What Users See
Flowcharts, sequence diagrams, and other Mermaid diagrams that match the site's color scheme and respond to theme changes.

### How It Works

**Conditional loading -- only pages with diagrams load Mermaid:**
```javascript
const mermaidBlocks = document.querySelectorAll('.prose .mermaid');
if (mermaidBlocks.length === 0) return;
```

**Dynamic ESM import from CDN:**
```javascript
const { default: mermaid } = await import(
  'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs'
);
```

**Theme-aware configuration:**
```javascript
function getMermaidConfig() {
  const s = getComputedStyle(document.documentElement);
  return {
    startOnLoad: false,
    securityLevel: 'strict',
    htmlLabels: false,
    flowchart: { curve: 'linear' },
    theme: 'base',
    themeVariables: {
      primaryColor:       s.getPropertyValue('--bg-strong').trim(),
      primaryTextColor:   s.getPropertyValue('--fg').trim(),
      primaryBorderColor: s.getPropertyValue('--line-strong').trim(),
      lineColor:          s.getPropertyValue('--line-strong').trim(),
      secondaryColor:     s.getPropertyValue('--bg-muted').trim(),
      tertiaryColor:      s.getPropertyValue('--bg').trim(),
      fontFamily:         "'IBM Plex Mono', monospace",
    },
  };
}
```

**Render and watch for theme changes:**
```javascript
mermaid.initialize(getMermaidConfig());
await mermaid.run({ nodes: mermaidBlocks });

// Re-render when theme toggles
new MutationObserver(async () => {
  mermaid.initialize(getMermaidConfig());
  // Clear and re-render each block
  for (const block of mermaidBlocks) {
    const source = block.getAttribute('data-mermaid-source');
    block.innerHTML = source;
  }
  await mermaid.run({ nodes: mermaidBlocks });
}).observe(document.documentElement, {
  attributes: true,
  attributeFilter: ['data-theme'],
});
```

**In Markdown, authors write:**
```markdown
```mermaid
flowchart TD
    A[User writes CLAUDE.md] --> B[Agent reads rules]
    B --> C[Agent generates code]
    C --> D[Code follows rules]
    D --> A
`` `
```

### Why This Approach
- **No build-time Mermaid plugin needed** -- simpler build, fewer dependencies
- **Theme-reactive** -- diagrams always match the current color scheme
- **Zero cost for non-diagram pages** -- conditional loading
- **CDN-hosted** -- Mermaid.js is ~2MB; loading from jsDelivr means browser caching across sites

---

## 3. Code Block Highlighting

### Shiki (Standard Pages)

**What happens at build time:**

Astro processes fenced code blocks through Shiki, which tokenizes the code using TextMate grammars and assigns dual-theme colors:

```html
<!-- Input markdown -->
```rust
fn main() {
    println!("hello");
}
`` `

<!-- Output HTML -->
<pre class="astro-code" style="background-color:var(--shiki-light-bg);...">
  <code>
    <span class="line">
      <span style="--shiki-light:#D73A49;--shiki-dark:#F97583">fn</span>
      <span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0"> main</span>
      <span style="--shiki-light:#24292E;--shiki-dark:#E1E4E8">()</span>
      <span style="--shiki-light:#24292E;--shiki-dark:#E1E4E8"> {</span>
    </span>
    <!-- ... more lines ... -->
  </code>
</pre>
```

**CSS switches the active color set:**
```css
[data-theme="light"] .astro-code span {
  color: var(--shiki-light) !important;
}
[data-theme="dark"] .astro-code span {
  color: var(--shiki-dark) !important;
}
```

### Custom Highlighting (Learn Lessons)

The lesson pages use manually crafted HTML with semantic CSS classes:

```html
<pre class="code-block"><code>
<span class="kw">const</span> <span class="fn">bootSequence</span> = <span class="kw">async</span> () => {
  <span class="cm">// Load configuration</span>
  <span class="kw">const</span> config = <span class="kw">await</span> <span class="fn">loadConfig</span>();
  <span class="kw">return</span> config.<span class="fn">initialize</span>();
};
</code></pre>
```

```css
.code-block .kw  { color: var(--code-keyword); }    /* Keywords */
.code-block .fn  { color: var(--code-function); }   /* Functions */
.code-block .str { color: var(--code-string); }     /* Strings */
.code-block .num { color: var(--code-number); }     /* Numbers */
.code-block .cm  { color: var(--code-comment); }    /* Comments */
.code-block .op  { color: var(--code-operator); }   /* Operators */
.code-block .ty  { color: var(--code-type); }       /* Types */
```

**Why two systems?** The lessons needed precise control over which tokens are highlighted and how. Shiki is automatic but opinionated. Manual classes let the author highlight exactly the parts that matter for teaching.

---

## 4. Scroll Reveal Animations

### What Users See
Page elements fade and slide into view as the user scrolls down.

### How It Works

```javascript
// Elements opt in with the .reveal class
const reveals = document.querySelectorAll('.reveal');

// Respect user preference
if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
  reveals.forEach(el => el.classList.add('revealed'));
  return;
}

const observer = new IntersectionObserver((entries) => {
  entries.forEach((entry, index) => {
    if (entry.isIntersecting) {
      // Staggered delay: 70ms per element, capped at 8
      const delay = Math.min(index, 8) * 70;
      setTimeout(() => {
        entry.target.classList.add('revealed');
      }, delay);
      observer.unobserve(entry.target);
    }
  });
}, { threshold: 0.1 });

reveals.forEach(el => observer.observe(el));
```

```css
.reveal {
  opacity: 0;
  transform: translateY(20px);
  transition: opacity 0.6s ease, transform 0.6s ease;
}
.reveal.revealed {
  opacity: 1;
  transform: translateY(0);
}
```

---

## 5. Interactive Quizzes (Learn Lessons)

### What Users See
Multiple-choice questions with radio buttons. A "Check Answer" button reveals whether the selection is correct.

### How It Works

```html
<div class="quiz" data-correct="2">
  <p class="quiz-question">What file does Claude Code read first on startup?</p>
  <label><input type="radio" name="q1" value="1"> package.json</label>
  <label><input type="radio" name="q1" value="2"> CLAUDE.md</label>
  <label><input type="radio" name="q1" value="3"> .env</label>
  <button class="quiz-check">Check Answer</button>
  <p class="quiz-feedback correct" hidden>Correct! CLAUDE.md is loaded during boot.</p>
  <p class="quiz-feedback incorrect" hidden>Not quite. Try again.</p>
</div>
```

```javascript
document.querySelectorAll('.quiz').forEach(quiz => {
  const correct = quiz.dataset.correct;
  const btn = quiz.querySelector('.quiz-check');

  btn.addEventListener('click', () => {
    const selected = quiz.querySelector('input:checked');
    if (!selected) return;

    const isCorrect = selected.value === correct;
    quiz.querySelector('.correct').hidden = !isCorrect;
    quiz.querySelector('.incorrect').hidden = isCorrect;
  });
});
```

---

## 6. Terminal Boot Animation (Learn Landing Page)

### What Users See
A simulated terminal boot sequence with text appearing line by line, mimicking a system startup.

### How It Works
Sequential `setTimeout` calls reveal pre-rendered HTML lines with a typewriter-like stagger effect. The animation runs once on page load and does not repeat.

---

## 7. Expandable Deep Dives

### What Users See
Collapsible sections labeled "Deep dive" that expand to show additional detail.

### Implementation
Native HTML `<details>` / `<summary>` -- no JavaScript needed:

```html
<details class="deep-dive">
  <summary>Deep dive: How tool dispatch works</summary>
  <div class="deep-dive-content">
    <p>The tool dispatch system uses a registry pattern...</p>
    <!-- Can contain code blocks, Mermaid diagrams, etc. -->
  </div>
</details>
```

---

## Summary: Feature Complexity Budget

| Feature | Build-time | Client JS | Library |
|---------|-----------|-----------|---------|
| Theme toggle | CSS variables | ~20 lines | None |
| Code highlighting | Shiki | 0 lines (CSS only) | Astro built-in |
| Mermaid diagrams | Pass-through | ~50 lines | Mermaid CDN |
| Scroll reveal | CSS classes | ~20 lines | None |
| Quizzes | HTML structure | ~15 lines | None |
| Boot animation | HTML | ~30 lines | None |
| Deep dives | `<details>` | 0 lines | None |

Total custom JavaScript on any given page: **under 150 lines**. The site achieves rich interactivity with minimal client-side code by leveraging CSS custom properties, native HTML elements, and build-time processing.
