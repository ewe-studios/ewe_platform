# Build Your Own -- Project Setup

## Prerequisites

- Node.js 20+ (for Astro and analysis scripts)
- An LLM API key (Anthropic or OpenAI)
- A codebase you want to document

## Step 1: Create the Astro Site

```bash
# Create new Astro project
npm create astro@latest site -- --template minimal --typescript strict

cd site

# Install dependencies
npm install -D tailwindcss @astrojs/tailwind
npm install -D @tailwindcss/typography
```

## Step 2: Configure Astro

```javascript
// site/astro.config.mjs
import { defineConfig } from 'astro/config';
import tailwind from '@astrojs/tailwind';

export default defineConfig({
  integrations: [tailwind()],
  markdown: {
    shikiConfig: {
      themes: {
        light: 'github-light',
        dark: 'github-dark',
      },
    },
  },
  site: 'https://your-docs-site.example.com',
});
```

## Step 3: Set Up Content Collections

```typescript
// site/src/content/config.ts
import { defineCollection, z } from 'astro:content';

const modules = defineCollection({
  type: 'content',
  schema: z.object({
    title: z.string(),
    description: z.string(),
    module_path: z.string(),
    layer: z.string(),
    dependencies: z.array(z.string()).default([]),
    dependents: z.array(z.string()).default([]),
    tags: z.array(z.string()).default([]),
    generated_at: z.string(),
  }),
});

const connections = defineCollection({
  type: 'content',
  schema: z.object({
    title: z.string(),
    description: z.string(),
    from_module: z.string(),
    to_module: z.string(),
    connection_type: z.enum(['imports', 'calls', 'implements', 'extends']),
    generated_at: z.string(),
  }),
});

const overviews = defineCollection({
  type: 'content',
  schema: z.object({
    title: z.string(),
    description: z.string(),
    generated_at: z.string(),
  }),
});

export const collections = { modules, connections, overviews };
```

## Step 4: Create the Base Layout

```astro
---
// site/src/layouts/Base.astro
interface Props {
  title: string;
  description?: string;
}

const { title, description } = Astro.props;
---
<!DOCTYPE html>
<html lang="en" data-theme="light">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <meta name="generator" content={Astro.generator} />
  <title>{title}</title>
  {description && <meta name="description" content={description} />}
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
  <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap" rel="stylesheet" />
  <script is:inline>
    (function() {
      const saved = localStorage.getItem('theme');
      const preferred = window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
      document.documentElement.setAttribute('data-theme', saved || preferred);
    })();
  </script>
</head>
<body class="bg-[var(--bg)] text-[var(--fg)] font-sans min-h-screen">
  <a href="#main" class="sr-only focus:not-sr-only">Skip to content</a>

  <header class="border-b border-[var(--line)] px-6 py-4">
    <nav class="max-w-4xl mx-auto flex items-center justify-between">
      <a href="/" class="font-mono text-sm font-medium">~/docs</a>
      <div class="flex items-center gap-4">
        <a href="/modules" class="text-sm text-[var(--fg-muted)] hover:text-[var(--fg)]">Modules</a>
        <a href="/connections" class="text-sm text-[var(--fg-muted)] hover:text-[var(--fg)]">Connections</a>
        <a href="/overview" class="text-sm text-[var(--fg-muted)] hover:text-[var(--fg)]">Overview</a>
        <button id="theme-toggle" class="text-sm font-mono text-[var(--fg-muted)]" aria-label="Toggle theme">
          theme
        </button>
      </div>
    </nav>
  </header>

  <main id="main" class="max-w-4xl mx-auto px-6 py-12">
    <slot />
  </main>

  <footer class="border-t border-[var(--line)] px-6 py-8 mt-20">
    <div class="max-w-4xl mx-auto text-sm text-[var(--fg-soft)]">
      Generated documentation. Powered by LLM analysis.
    </div>
  </footer>

  <script is:inline>
    document.getElementById('theme-toggle')?.addEventListener('click', () => {
      const current = document.documentElement.getAttribute('data-theme');
      const next = current === 'dark' ? 'light' : 'dark';
      document.documentElement.setAttribute('data-theme', next);
      localStorage.setItem('theme', next);
    });
  </script>
</body>
</html>
```

## Step 5: Set Up CSS Design Tokens

```css
/* site/src/styles/global.css */
@import "tailwindcss";

@layer base {
  [data-theme="light"] {
    --bg:          #fafaf9;
    --bg-strong:   #ffffff;
    --bg-muted:    #f0efed;
    --fg:          #1c1917;
    --fg-muted:    #57534e;
    --fg-soft:     #a8a29e;
    --accent:      #2563eb;
    --line:        #e7e5e4;
    --line-strong: #d6d3d1;
  }

  [data-theme="dark"] {
    --bg:          #0c0a09;
    --bg-strong:   #1c1917;
    --bg-muted:    #292524;
    --fg:          #fafaf9;
    --fg-muted:    #a8a29e;
    --fg-soft:     #78716c;
    --accent:      #60a5fa;
    --line:        #292524;
    --line-strong: #44403c;
  }

  body {
    font-family: 'Inter', system-ui, sans-serif;
  }

  code, pre {
    font-family: 'JetBrains Mono', monospace;
  }
}

/* Shiki theme switching */
[data-theme="light"] .astro-code span {
  color: var(--shiki-light) !important;
  background-color: var(--shiki-light-bg) !important;
}
[data-theme="dark"] .astro-code span {
  color: var(--shiki-dark) !important;
  background-color: var(--shiki-dark-bg) !important;
}
[data-theme="light"] .astro-code {
  background-color: var(--bg-strong) !important;
}
[data-theme="dark"] .astro-code {
  background-color: var(--bg-strong) !important;
}

/* Prose styling for generated content */
.prose h1 { font-size: 2rem; font-weight: 700; margin-bottom: 1rem; }
.prose h2 { font-size: 1.5rem; font-weight: 600; margin-top: 2.5rem; margin-bottom: 0.75rem; }
.prose h3 { font-size: 1.25rem; font-weight: 600; margin-top: 2rem; margin-bottom: 0.5rem; }
.prose p  { margin-bottom: 1rem; line-height: 1.75; }
.prose ul { list-style: disc; padding-left: 1.5rem; margin-bottom: 1rem; }
.prose ol { list-style: decimal; padding-left: 1.5rem; margin-bottom: 1rem; }
.prose a  { color: var(--accent); text-decoration: underline; }

.prose pre {
  padding: 1rem;
  border-radius: 0.5rem;
  overflow-x: auto;
  margin-bottom: 1.5rem;
  border: 1px solid var(--line);
}

.prose code:not(pre code) {
  background: var(--bg-muted);
  padding: 0.15rem 0.35rem;
  border-radius: 0.25rem;
  font-size: 0.875em;
}

.prose table {
  width: 100%;
  border-collapse: collapse;
  margin-bottom: 1.5rem;
}
.prose th, .prose td {
  border: 1px solid var(--line);
  padding: 0.5rem 0.75rem;
  text-align: left;
}
.prose th {
  background: var(--bg-muted);
  font-weight: 600;
}
```

## Step 6: Create Page Templates

```astro
---
// site/src/pages/modules/[...slug].astro
import { getCollection } from 'astro:content';
import Base from '../../layouts/Base.astro';

export async function getStaticPaths() {
  const modules = await getCollection('modules');
  return modules.map((entry) => ({
    params: { slug: entry.slug },
    props: { entry },
  }));
}

const { entry } = Astro.props;
const { Content } = await entry.render();
---
<Base title={entry.data.title} description={entry.data.description}>
  <article class="prose">
    <header class="mb-8">
      <p class="text-sm font-mono text-[var(--fg-soft)] mb-2">{entry.data.module_path}</p>
      <h1>{entry.data.title}</h1>
      <p class="text-[var(--fg-muted)]">{entry.data.description}</p>
      {entry.data.tags.length > 0 && (
        <div class="flex gap-2 mt-3">
          {entry.data.tags.map(tag => (
            <span class="text-xs font-mono px-2 py-0.5 rounded bg-[var(--bg-muted)] text-[var(--fg-muted)]">
              {tag}
            </span>
          ))}
        </div>
      )}
    </header>

    <Content />

    {(entry.data.dependencies.length > 0 || entry.data.dependents.length > 0) && (
      <footer class="mt-12 pt-8 border-t border-[var(--line)]">
        {entry.data.dependencies.length > 0 && (
          <div class="mb-4">
            <h3 class="text-sm font-mono font-medium text-[var(--fg-muted)] mb-2">Depends on</h3>
            <div class="flex flex-wrap gap-2">
              {entry.data.dependencies.map(dep => (
                <a href={`/modules/${dep}`} class="text-sm text-[var(--accent)]">{dep}</a>
              ))}
            </div>
          </div>
        )}
        {entry.data.dependents.length > 0 && (
          <div>
            <h3 class="text-sm font-mono font-medium text-[var(--fg-muted)] mb-2">Used by</h3>
            <div class="flex flex-wrap gap-2">
              {entry.data.dependents.map(dep => (
                <a href={`/modules/${dep}`} class="text-sm text-[var(--accent)]">{dep}</a>
              ))}
            </div>
          </div>
        )}
      </footer>
    )}
  </article>
</Base>
```

## Step 7: Set Up the Analysis Scripts Directory

```bash
# Back in the project root
mkdir -p analyze generate/prompts scripts
```

```json
// analyze/package.json (or root package.json)
{
  "type": "module",
  "scripts": {
    "analyze": "node analyze/extract-structure.ts",
    "generate": "node generate/generate-docs.ts",
    "build": "cd site && npm run build",
    "pipeline": "./scripts/full-pipeline.sh"
  },
  "dependencies": {
    "@anthropic-ai/sdk": "^0.52.0",
    "glob": "^11.0.0"
  },
  "devDependencies": {
    "tsx": "^4.19.0",
    "typescript": "^5.7.0"
  }
}
```

## Step 8: Create the Pipeline Script

```bash
#!/usr/bin/env bash
# scripts/full-pipeline.sh

set -euo pipefail

CODEBASE_PATH="${1:?Usage: ./scripts/full-pipeline.sh /path/to/codebase}"

echo "=== Step 1: Analyzing codebase ==="
npx tsx analyze/extract-structure.ts "$CODEBASE_PATH"
npx tsx analyze/extract-deps.ts "$CODEBASE_PATH"
npx tsx analyze/extract-symbols.ts "$CODEBASE_PATH"

echo "=== Step 2: Generating documentation ==="
npx tsx generate/generate-docs.ts

echo "=== Step 3: Building site ==="
cd site && npm run build

echo "=== Done. Output in site/dist/ ==="
```

## What You Have After Setup

A project skeleton with:
- An Astro site configured with Tailwind, Shiki dual-theme code highlighting, content collections, and a base layout
- Directory structure for analysis scripts, generation scripts, and prompt templates
- A pipeline script that ties everything together
- No code highlighting or Mermaid integration yet -- those come in the next sections
