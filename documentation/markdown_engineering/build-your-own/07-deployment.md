# Build Your Own -- Deployment and Automation

## Building the Static Site

```bash
cd site && npm run build
```

Astro outputs static HTML, CSS, and JS to `site/dist/`. This directory is the complete site -- no server needed.

## Hosting Options

### Vercel (Recommended for Simplicity)

```bash
npm install -g vercel
cd site && vercel
```

Vercel auto-detects Astro and configures the build. Subsequent deployments happen on `git push` if you connect a repository.

### Netlify

```toml
# site/netlify.toml
[build]
  command = "npm run build"
  publish = "dist"

[build.environment]
  NODE_VERSION = "20"
```

### Cloudflare Pages

```bash
# In Cloudflare Pages dashboard:
# Build command: npm run build
# Build output directory: dist
# Root directory: site
```

### GitHub Pages

```yaml
# .github/workflows/deploy.yml
name: Deploy Documentation Site

on:
  push:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - run: npm ci
        working-directory: site
      - run: npm run build
        working-directory: site
      - uses: actions/upload-pages-artifact@v3
        with:
          path: site/dist

  deploy:
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - uses: actions/deploy-pages@v4
        id: deployment
```

### Self-Hosted (Nginx)

```nginx
server {
    listen 80;
    server_name docs.yourproject.com;

    root /var/www/docs/dist;
    index index.html;

    location / {
        try_files $uri $uri/ $uri.html =404;
    }

    # Cache static assets
    location ~* \.(css|js|png|jpg|svg|woff2)$ {
        expires 1y;
        add_header Cache-Control "public, immutable";
    }
}
```

## Full Pipeline Automation

### CI Pipeline: Regenerate on Codebase Changes

```yaml
# .github/workflows/generate-docs.yml
name: Generate Documentation

on:
  push:
    branches: [main]
    paths:
      - 'src/**'
      - 'lib/**'

  # Allow manual trigger
  workflow_dispatch:

jobs:
  generate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: 20

      - run: npm ci
        working-directory: analyze

      - run: npm ci
        working-directory: generate

      - run: npm ci
        working-directory: site

      # Step 1: Analyze codebase
      - name: Extract structure
        run: npx tsx analyze/extract-structure.ts .

      - name: Extract dependencies
        run: npx tsx analyze/extract-deps.ts .

      - name: Extract symbols
        run: npx tsx analyze/extract-symbols.ts .

      # Step 2: Generate docs (needs API key)
      - name: Generate documentation
        run: npx tsx generate/generate-docs.ts
        env:
          ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}

      # Step 3: Build site
      - name: Build site
        run: cd site && npm run build

      # Step 4: Deploy
      - uses: actions/upload-pages-artifact@v3
        with:
          path: site/dist

  deploy:
    needs: generate
    runs-on: ubuntu-latest
    permissions:
      pages: write
      id-token: write
    environment:
      name: github-pages
    steps:
      - uses: actions/deploy-pages@v4
```

### Local Development: Watch Mode

```bash
#!/usr/bin/env bash
# scripts/dev.sh
# Run the Astro dev server with hot reload for content changes

cd site && npm run dev
```

During development, edit markdown files in `site/src/content/` and Astro hot-reloads the browser. Use this flow:

1. Run `npm run dev` in the site directory
2. Run the generation script against your codebase
3. Generated markdown files appear in `site/src/content/`
4. Astro picks them up and the browser refreshes
5. Review the output, adjust prompts, regenerate

### Makefile for Common Operations

```makefile
# Makefile

CODEBASE ?= ../your-project

.PHONY: analyze generate build deploy dev clean pipeline

analyze:
	npx tsx analyze/extract-structure.ts $(CODEBASE)
	npx tsx analyze/extract-deps.ts $(CODEBASE)
	npx tsx analyze/extract-symbols.ts $(CODEBASE)

generate:
	npx tsx generate/generate-docs.ts

build:
	cd site && npm run build

dev:
	cd site && npm run dev

pipeline: analyze generate build

deploy: pipeline
	cd site && npx vercel --prod

clean:
	rm -rf analyze/output site/dist site/src/content/modules site/src/content/connections
```

Usage:

```bash
make pipeline CODEBASE=/path/to/your/project
make dev
make deploy
```

## Keeping Documentation Fresh

### Strategy 1: CI-Triggered Regeneration

Set up the GitHub Actions workflow above to regenerate on every push to `src/`. This ensures documentation is never more than one commit behind.

**Trade-off:** Each regeneration costs LLM API credits. For active repositories with many daily pushes, costs add up.

### Strategy 2: Scheduled Regeneration

```yaml
on:
  schedule:
    - cron: '0 6 * * 1'  # Every Monday at 6 AM
```

Regenerate weekly. The site shows the `generated_at` timestamp so readers know how current the docs are.

**Trade-off:** Documentation can be up to a week stale. Acceptable for most projects.

### Strategy 3: Manual Trigger with Diff Review

The workflow includes `workflow_dispatch` for manual triggers. Combine with a PR-based flow:

1. Manual trigger creates a branch with regenerated docs
2. Open a PR showing the documentation diff
3. Review the LLM-generated changes
4. Merge to deploy

```yaml
- name: Create documentation PR
  if: github.event_name == 'workflow_dispatch'
  run: |
    git checkout -b docs/regenerate-$(date +%Y%m%d)
    git add site/src/content/
    git commit -m "docs: regenerate from current codebase"
    gh pr create --title "docs: regenerate documentation" \
      --body "Automated documentation regeneration from current codebase state."
  env:
    GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

### Strategy 4: Incremental with Git Diff

Only regenerate modules whose source files changed since the last generation:

```bash
#!/usr/bin/env bash
# scripts/incremental.sh

LAST_GEN_COMMIT=$(cat .last-docs-generation 2>/dev/null || echo "HEAD~1")

# Get changed source files
CHANGED=$(git diff --name-only "$LAST_GEN_COMMIT" HEAD -- src/ lib/)

if [ -z "$CHANGED" ]; then
  echo "No source changes since last generation. Skipping."
  exit 0
fi

echo "Changed files:"
echo "$CHANGED"

# Pass changed files to the generation script
echo "$CHANGED" > analyze/output/changed-files.txt
npx tsx generate/generate-docs.ts --incremental

# Record this generation point
git rev-parse HEAD > .last-docs-generation
```

## Performance Optimization

### Astro Build Performance

For large documentation sites (100+ pages):

```javascript
// site/astro.config.mjs
export default defineConfig({
  build: {
    // Inline stylesheets under 4KB
    inlineStylesheets: 'auto',
  },
  // Prefetch linked pages for faster navigation
  prefetch: {
    prefetchAll: true,
    defaultStrategy: 'viewport',
  },
});
```

### CDN Caching

All static assets can be cached aggressively. Set long cache headers for CSS, JS, and fonts. HTML pages should use shorter cache times or `stale-while-revalidate`:

```
# HTML: cache for 1 hour, serve stale while revalidating
Cache-Control: public, max-age=3600, stale-while-revalidate=86400

# Assets (CSS, JS, fonts, images): cache for 1 year (Astro uses content hashes)
Cache-Control: public, max-age=31536000, immutable
```

## Production Checklist

Before deploying to production:

- [ ] Site builds without errors: `cd site && npm run build`
- [ ] All content collection schemas validate (build would fail if not)
- [ ] Mermaid diagrams render in both light and dark themes
- [ ] Code blocks highlight correctly for all languages used
- [ ] Cross-reference links (`/modules/X`) resolve to real pages
- [ ] `generated_at` timestamps are present in frontmatter
- [ ] Mobile layout works (test at 375px width)
- [ ] `robots.txt` is present if you want search indexing
- [ ] `sitemap.xml` is generated (Astro has a sitemap integration)
- [ ] OpenGraph meta tags are present for social sharing

### Add Sitemap

```bash
cd site && npx astro add sitemap
```

```javascript
// site/astro.config.mjs
import sitemap from '@astrojs/sitemap';

export default defineConfig({
  site: 'https://docs.yourproject.com',
  integrations: [tailwind(), sitemap()],
});
```

## Summary: What You've Built

After following this guide, you have:

1. **Analysis scripts** that extract structure, dependencies, and symbols from any codebase
2. **A generation script** that uses an LLM to produce contextual Markdown documentation with code snippets, Mermaid diagrams, and cross-references
3. **An Astro site** with Tailwind CSS, Shiki code highlighting, Mermaid diagram rendering, dark/light theming, and content collection navigation
4. **A pipeline** that chains analysis → generation → build into a single command
5. **Deployment automation** via CI/CD to any static hosting provider
6. **Incremental regeneration** so you only pay LLM costs for modules that actually changed

The total custom code is under 1,000 lines. The architecture has four layers, each with one job, communicating via files. No databases, no custom servers, no framework lock-in.

Anyone can fork this, point it at their codebase, and have a documentation site in an afternoon.
