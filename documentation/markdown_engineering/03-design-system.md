# markdown.engineering -- Design System

## Visual Identity

The site deliberately avoids the standard blue/purple tech palette. Instead, it uses warm, earthy tones that evoke parchment, leather, and wood -- a nod to the "engineering" in the name, suggesting craft and materiality rather than sterile digital aesthetics.

The `~/` glyph is used as the site's brand prefix, evoking a terminal home directory path. This ties the visual identity back to the CLI-centric philosophy.

## Color Tokens

### Light Theme

```css
[data-theme="light"] {
  /* Backgrounds */
  --bg:        #f8f1e6;   /* Warm parchment */
  --bg-strong: #fff8ef;   /* Lighter parchment for elevated surfaces */
  --bg-muted:  #f0e2cd;   /* Deeper parchment for recessed areas */

  /* Foregrounds */
  --fg:        #2b2018;   /* Dark brown, primary text */
  --fg-muted:  #5f4d40;   /* Medium brown, secondary text */
  --fg-soft:   #8f7a67;   /* Light brown, tertiary text */

  /* Accent */
  --accent:    #a36540;   /* Burnt sienna, links and highlights */

  /* Lines */
  --line:        #ddcbb4; /* Light borders */
  --line-strong: #c1a78a; /* Emphasized borders */
}
```

### Dark Theme

```css
[data-theme="dark"] {
  /* Backgrounds */
  --bg:        #13100f;   /* Near-black with warm undertone */
  --bg-strong: #1f1a17;   /* Slightly elevated dark surface */
  --bg-muted:  #29231f;   /* Recessed dark surface */

  /* Foregrounds */
  --fg:        #f7efe4;   /* Warm white, primary text */
  --fg-muted:  #d8c5b0;   /* Muted warm, secondary text */
  --fg-soft:   #b19780;   /* Soft warm, tertiary text */

  /* Accent */
  --accent:    #d4a176;   /* Gold/amber, links and highlights */

  /* Lines */
  --line:        #3d332c; /* Subtle dark borders */
  --line-strong: #5a493f; /* Visible dark borders */
}
```

### Background Effect

The body background uses radial gradient overlays from the accent color at low opacity, creating a subtle ambient glow:

```css
body {
  background:
    radial-gradient(ellipse at 8%  50%, var(--accent) / 0.04, transparent 50%),
    radial-gradient(ellipse at 92% 50%, var(--accent) / 0.03, transparent 50%),
    radial-gradient(ellipse at 50% 50%, var(--accent) / 0.02, transparent 70%),
    var(--bg);
}
```

## Typography

### Font Stack

```css
/* Headings */
h1, h2, h3 {
  font-family: 'Fraunces', serif;
  font-optical-sizing: auto;
}

/* Body */
body {
  font-family: 'Manrope', sans-serif;
}

/* Code */
code, pre {
  font-family: 'IBM Plex Mono', monospace;
}

/* Learn/Play sections use alternate fonts */
.learn-page, .play-page {
  font-family: 'Space Grotesk', sans-serif;
}
.learn-page code, .play-page code {
  font-family: 'JetBrains Mono', monospace;
}
```

### Type Scale and Weight

| Element | Font | Weight | Purpose |
|---------|------|--------|---------|
| h1 | Fraunces | 700 | Page titles |
| h2 | Fraunces | 600 | Section heads |
| h3 | Fraunces | 500 | Subsection heads |
| Body | Manrope | 400 | Paragraph text |
| Bold body | Manrope | 700 | Emphasis |
| Code | IBM Plex Mono | 400 | Inline code |
| Code labels | IBM Plex Mono | 500 | Code block headers |

## Component Patterns

### Surface

The `.surface` class is the primary container pattern:

```css
.surface {
  border: 1px solid var(--line);
  border-radius: 0.75rem;
  background: color-mix(in srgb, var(--bg-strong), transparent 10%);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.08);
}
```

### Terminal Hero

A macOS-style terminal window header component used on the main page:

```html
<terminal-hero>
  <div class="terminal-header">
    <span class="dot red"></span>
    <span class="dot yellow"></span>
    <span class="dot green"></span>
  </div>
  <div class="terminal-body">
    <!-- Content rendered inside terminal chrome -->
  </div>
</terminal-hero>
```

### Callout Boxes (Learn Lessons)

Three variants used in the course content:

```html
<!-- Info callout -->
<div class="callout info">
  <strong>Key concept:</strong> Explanation text here.
</div>

<!-- Tip callout -->
<div class="callout tip">
  <strong>Tip:</strong> Helpful suggestion here.
</div>

<!-- Warning callout -->
<div class="callout warn">
  <strong>Warning:</strong> Important caveat here.
</div>
```

### Phase Grids

Card-based layouts for presenting multi-step concepts:

```html
<div class="phase-grid">
  <div class="phase-card">
    <h4>Phase 1: Discovery</h4>
    <p>Description of what happens in this phase.</p>
  </div>
  <div class="phase-card">
    <h4>Phase 2: Analysis</h4>
    <p>Description of analysis phase.</p>
  </div>
</div>
```

### Chapter Tags (Learn Lessons)

Color-coded tags for organizing the 50-lesson course:

```css
.tag-arch   { color: orange; }    /* Core Architecture */
.tag-tools  { color: green;  }    /* Tool System */
.tag-agents { color: blue;   }    /* Agent Intelligence */
.tag-ui     { color: gold;   }    /* The Interface */
.tag-infra  { color: brown;  }    /* Infrastructure */
.tag-net    { color: teal;   }    /* Connectivity */
.tag-leak   { color: red;    }    /* Unreleased */
.tag-meta   { color: gray;   }    /* The Big Picture */
```

## Responsive Breakpoints

```css
/* Mobile-first, key breakpoints */
@media (min-width: 640px)  { /* Tablet: show/hide elements */ }
@media (min-width: 760px)  { /* Small desktop: grid layout shifts */ }
@media (min-width: 1040px) { /* Desktop: full layout */ }
```

## Accessibility

- Skip-to-content link on every page
- Animations respect `prefers-reduced-motion`
- Theme toggle respects `prefers-color-scheme` as default
- Semantic HTML throughout (nav, main, article, aside, footer)
- Font `display=swap` prevents invisible text during load

## What Makes This Design System Replicable

1. **CSS custom properties as the single source of truth** -- Every color, spacing, and border value flows from a small set of tokens. Changing 12 variables transforms the entire site.
2. **No component library dependency** -- Everything is plain CSS. No styled-components, no CSS-in-JS, no Chakra/MUI.
3. **Progressive enhancement** -- The site works without JavaScript. Themes, animations, and Mermaid are enhancements layered on top.
4. **Warm, distinctive palette** -- Proves that documentation sites don't need to look like every other tech site. The earthy tones create a memorable identity with minimal effort.
