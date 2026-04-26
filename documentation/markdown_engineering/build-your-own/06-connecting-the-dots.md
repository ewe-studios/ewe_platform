# Build Your Own -- Connecting the Dots

## The Problem

Individual module documentation is useful but incomplete. A developer reading about the auth middleware needs to understand how it fits into the request lifecycle, which other modules depend on its output, and what happens when it fails. Isolated pages create isolated understanding.

This section covers how to build cross-references, dependency graphs, and navigation structures that connect modules into a coherent whole.

## Dependency Graph Extraction

### For TypeScript/JavaScript Projects

```typescript
// analyze/extract-deps.ts
import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { glob } from 'glob';
import { dirname, relative, resolve } from 'path';

interface Dependency {
  from: string;
  to: string;
  type: 'imports';
  importedSymbols: string[];
  fromFile: string;
  toFile: string;
}

async function extractDependencies(codebasePath: string): Promise<Dependency[]> {
  const files = await glob('**/*.{ts,tsx,js,jsx}', {
    cwd: codebasePath,
    ignore: ['node_modules/**', 'dist/**', '**/*.test.*', '**/*.spec.*'],
  });

  const deps: Dependency[] = [];

  for (const file of files) {
    const content = readFileSync(`${codebasePath}/${file}`, 'utf-8');
    const lines = content.split('\n');

    for (const line of lines) {
      // Match: import { X, Y } from './path'
      // Match: import X from './path'
      // Match: import * as X from './path'
      const importMatch = line.match(
        /import\s+(?:(\{[^}]+\})|(\w+)|\*\s+as\s+(\w+))\s+from\s+['"]([^'"]+)['"]/
      );

      if (!importMatch) continue;

      const importPath = importMatch[4];

      // Only track local imports, not node_modules
      if (!importPath.startsWith('.') && !importPath.startsWith('/')) continue;

      // Resolve the import to an actual file
      const fromDir = dirname(resolve(codebasePath, file));
      const resolved = resolveImport(fromDir, importPath, codebasePath);
      if (!resolved) continue;

      const importedSymbols = importMatch[1]
        ? importMatch[1].replace(/[{}]/g, '').split(',').map(s => s.trim())
        : importMatch[2] ? [importMatch[2]] : ['*'];

      const fromModule = fileToModule(file);
      const toModule = fileToModule(relative(codebasePath, resolved));

      if (fromModule === toModule) continue;

      deps.push({
        from: fromModule,
        to: toModule,
        type: 'imports',
        importedSymbols,
        fromFile: file,
        toFile: relative(codebasePath, resolved),
      });
    }
  }

  return deduplicateDeps(deps);
}

function fileToModule(filePath: string): string {
  // Convert file paths to module names
  // src/middleware/auth.ts -> auth-middleware
  // src/db/users.ts -> database
  // Customize this mapping for your project structure
  const parts = filePath.replace(/\.(ts|tsx|js|jsx)$/, '').split('/');

  // Remove 'src' prefix
  if (parts[0] === 'src') parts.shift();

  // Remove 'index' suffix
  if (parts[parts.length - 1] === 'index') parts.pop();

  return parts.join('-');
}

function resolveImport(fromDir: string, importPath: string, root: string): string | null {
  const extensions = ['.ts', '.tsx', '.js', '.jsx', '/index.ts', '/index.tsx', '/index.js'];

  for (const ext of extensions) {
    const candidate = resolve(fromDir, importPath + ext);
    try {
      readFileSync(candidate);
      return candidate;
    } catch { /* continue */ }
  }

  return null;
}

function deduplicateDeps(deps: Dependency[]): Dependency[] {
  const map = new Map<string, Dependency>();
  for (const dep of deps) {
    const key = `${dep.from}->${dep.to}`;
    const existing = map.get(key);
    if (existing) {
      existing.importedSymbols = [
        ...new Set([...existing.importedSymbols, ...dep.importedSymbols]),
      ];
    } else {
      map.set(key, { ...dep });
    }
  }
  return Array.from(map.values());
}

// Main
const codebasePath = process.argv[2];
if (!codebasePath) {
  console.error('Usage: npx tsx analyze/extract-deps.ts /path/to/codebase');
  process.exit(1);
}

const deps = await extractDependencies(codebasePath);
mkdirSync('analyze/output', { recursive: true });
writeFileSync('analyze/output/dependencies.json', JSON.stringify(deps, null, 2));
console.log(`Extracted ${deps.length} dependencies from ${codebasePath}`);
```

### For Rust Projects

```typescript
// analyze/extract-deps-rust.ts
import { execSync } from 'child_process';
import { writeFileSync, mkdirSync, readFileSync } from 'fs';
import { glob } from 'glob';

interface RustDependency {
  from: string;
  to: string;
  type: 'imports';
  importedSymbols: string[];
  fromFile: string;
}

async function extractRustDeps(codebasePath: string): Promise<RustDependency[]> {
  const deps: RustDependency[] = [];

  // Use cargo metadata for crate-level dependencies
  try {
    const metadata = JSON.parse(
      execSync('cargo metadata --format-version 1 --no-deps', {
        cwd: codebasePath,
        encoding: 'utf-8',
      })
    );

    // Extract workspace member dependencies
    for (const pkg of metadata.packages) {
      for (const dep of pkg.dependencies) {
        deps.push({
          from: pkg.name,
          to: dep.name,
          type: 'imports',
          importedSymbols: [],
          fromFile: `${pkg.name}/Cargo.toml`,
        });
      }
    }
  } catch { /* not a cargo project, fall through to file-level analysis */ }

  // File-level: scan use statements
  const files = await glob('**/*.rs', {
    cwd: codebasePath,
    ignore: ['target/**'],
  });

  for (const file of files) {
    const content = readFileSync(`${codebasePath}/${file}`, 'utf-8');
    const useMatches = content.matchAll(/^use\s+(crate|super|self)::(\w+)(?:::(.+))?;/gm);

    for (const match of useMatches) {
      const targetModule = match[2];
      const symbols = match[3] ? match[3].split('::').pop()! : '*';

      deps.push({
        from: file.replace(/\.rs$/, '').replace(/\//g, '::'),
        to: targetModule,
        type: 'imports',
        importedSymbols: [symbols],
        fromFile: file,
      });
    }
  }

  return deps;
}

const codebasePath = process.argv[2]!;
const deps = await extractRustDeps(codebasePath);
mkdirSync('analyze/output', { recursive: true });
writeFileSync('analyze/output/dependencies.json', JSON.stringify(deps, null, 2));
console.log(`Extracted ${deps.length} Rust dependencies`);
```

## Structure Extraction

```typescript
// analyze/extract-structure.ts
import { readFileSync, writeFileSync, mkdirSync, statSync } from 'fs';
import { glob } from 'glob';
import { dirname } from 'path';

interface Module {
  name: string;
  path: string;
  files: string[];
  layer: string;
  totalLines: number;
}

async function extractStructure(codebasePath: string): Promise<{ modules: Module[] }> {
  const files = await glob('**/*.{ts,tsx,js,jsx,rs,py}', {
    cwd: codebasePath,
    ignore: ['node_modules/**', 'dist/**', 'target/**', '.git/**',
             '**/*.test.*', '**/*.spec.*'],
  });

  // Group files into modules by directory
  const dirGroups = new Map<string, string[]>();

  for (const file of files) {
    const dir = dirname(file);
    const fullPath = `${codebasePath}/${file}`;

    if (!dirGroups.has(dir)) {
      dirGroups.set(dir, []);
    }
    dirGroups.get(dir)!.push(fullPath);
  }

  // Convert directory groups to modules
  const modules: Module[] = [];

  for (const [dir, moduleFiles] of dirGroups) {
    const name = dirToModuleName(dir);
    const layer = inferLayer(dir);
    const totalLines = moduleFiles.reduce((sum, f) => {
      return sum + readFileSync(f, 'utf-8').split('\n').length;
    }, 0);

    modules.push({
      name,
      path: dir,
      files: moduleFiles,
      layer,
      totalLines,
    });
  }

  return { modules };
}

function dirToModuleName(dir: string): string {
  return dir
    .replace(/^src\//, '')
    .replace(/\//g, '-')
    || 'root';
}

function inferLayer(dir: string): string {
  // Customize these patterns for your project
  if (dir.match(/middleware|guard|filter/i)) return 'middleware';
  if (dir.match(/route|controller|handler|api/i)) return 'api';
  if (dir.match(/db|database|repo|store/i)) return 'data';
  if (dir.match(/model|entity|schema|type/i)) return 'domain';
  if (dir.match(/util|helper|lib|common/i)) return 'utility';
  if (dir.match(/config|env|setting/i)) return 'config';
  if (dir.match(/service|worker|job/i)) return 'service';
  if (dir.match(/component|view|page|ui/i)) return 'ui';
  return 'core';
}

const codebasePath = process.argv[2]!;
const structure = await extractStructure(codebasePath);
mkdirSync('analyze/output', { recursive: true });
writeFileSync('analyze/output/structure.json', JSON.stringify(structure, null, 2));
console.log(`Found ${structure.modules.length} modules`);
```

## Building Navigation from Dependencies

The dependency graph enables three types of navigation in the rendered site:

### 1. Module Footer: Dependencies and Dependents

Every module page shows what it depends on and what depends on it (already covered in the page template in `01-project-setup.md`).

### 2. Module Index: Grouped by Layer

```astro
---
// site/src/pages/modules/index.astro
import { getCollection } from 'astro:content';
import Base from '../../layouts/Base.astro';

const modules = await getCollection('modules');

// Group by layer
const layers = modules.reduce((acc, mod) => {
  const layer = mod.data.layer;
  (acc[layer] = acc[layer] || []).push(mod);
  return acc;
}, {} as Record<string, typeof modules>);

// Order layers top-down (customize for your architecture)
const layerOrder = ['api', 'middleware', 'service', 'domain', 'data', 'config', 'utility'];
const sortedLayers = layerOrder
  .filter(l => layers[l])
  .map(l => [l, layers[l]] as const);
---
<Base title="Modules" description="All documented modules">
  <h1 class="text-3xl font-bold mb-8">Modules</h1>

  {sortedLayers.map(([layer, mods]) => (
    <section class="mb-10">
      <h2 class="text-xs font-mono uppercase tracking-wider text-[var(--fg-soft)] mb-3 border-b border-[var(--line)] pb-2">
        {layer}
      </h2>
      <div class="grid gap-3">
        {mods.map(mod => (
          <a href={`/modules/${mod.slug}`}
             class="block p-4 rounded-lg border border-[var(--line)] hover:border-[var(--accent)] transition-colors">
            <div class="flex items-baseline justify-between">
              <h3 class="font-medium">{mod.data.title}</h3>
              <span class="text-xs font-mono text-[var(--fg-soft)]">{mod.data.module_path}</span>
            </div>
            <p class="text-sm text-[var(--fg-muted)] mt-1">{mod.data.description}</p>
          </a>
        ))}
      </div>
    </section>
  ))}
</Base>
```

### 3. Full Architecture Diagram (Overview Page)

The LLM-generated architecture overview includes a Mermaid diagram with subgraphs per layer. This diagram is the visual map of the entire system.

### 4. Connections Index

```astro
---
// site/src/pages/connections/index.astro
import { getCollection } from 'astro:content';
import Base from '../../layouts/Base.astro';

const connections = await getCollection('connections');

// Group by from_module
const grouped = connections.reduce((acc, conn) => {
  const from = conn.data.from_module;
  (acc[from] = acc[from] || []).push(conn);
  return acc;
}, {} as Record<string, typeof connections>);
---
<Base title="Connections" description="How modules interact">
  <h1 class="text-3xl font-bold mb-8">Connections</h1>

  {Object.entries(grouped).map(([from, conns]) => (
    <section class="mb-8">
      <h2 class="font-medium text-lg mb-3">
        <a href={`/modules/${from}`} class="text-[var(--accent)]">{from}</a>
      </h2>
      <div class="grid gap-2 pl-4 border-l-2 border-[var(--line)]">
        {conns.map(conn => (
          <a href={`/connections/${conn.slug}`}
             class="block p-3 rounded hover:bg-[var(--bg-muted)] transition-colors">
            <span class="text-sm">
              → <span class="text-[var(--accent)]">{conn.data.to_module}</span>
            </span>
            <span class="text-xs text-[var(--fg-soft)] ml-2">({conn.data.connection_type})</span>
            <p class="text-sm text-[var(--fg-muted)] mt-0.5">{conn.data.description}</p>
          </a>
        ))}
      </div>
    </section>
  ))}
</Base>
```

## Breadcrumb Navigation

Show the user where they are in the hierarchy:

```astro
---
// site/src/components/Breadcrumbs.astro
interface Props {
  items: { label: string; href?: string }[];
}
const { items } = Astro.props;
---
<nav aria-label="Breadcrumb" class="text-sm text-[var(--fg-soft)] mb-6">
  <ol class="flex items-center gap-1.5">
    <li><a href="/" class="hover:text-[var(--fg)]">~/docs</a></li>
    {items.map((item, i) => (
      <li class="flex items-center gap-1.5">
        <span>/</span>
        {item.href ? (
          <a href={item.href} class="hover:text-[var(--fg)]">{item.label}</a>
        ) : (
          <span class="text-[var(--fg-muted)]">{item.label}</span>
        )}
      </li>
    ))}
  </ol>
</nav>
```

Usage in a module page:

```astro
<Breadcrumbs items={[
  { label: 'modules', href: '/modules' },
  { label: entry.data.title },
]} />
```

## Search (Optional but Valuable)

For larger documentation sites, add client-side search with Pagefind:

```bash
cd site && npx pagefind --site dist
```

Pagefind indexes your static HTML at build time and provides a search widget with zero server-side dependencies. Add it to your layout:

```html
<link href="/pagefind/pagefind-ui.css" rel="stylesheet" />
<script src="/pagefind/pagefind-ui.js"></script>
<div id="search"></div>
<script>
  new PagefindUI({ element: '#search', showSubResults: true });
</script>
```

## Key Principles

1. **Dependencies are first-class content** -- they get their own collection, their own pages, and their own navigation. Understanding how A connects to B is as important as understanding A alone.

2. **Navigation mirrors architecture** -- the module index groups by layer because that's how developers think about the system. The connection index groups by source module because that's how developers trace code.

3. **Every page is a jumping-off point** -- module pages link to dependencies. Connection pages link to both endpoints. The overview links to everything. No dead ends.

4. **The dependency graph is extracted, not hand-maintained** -- the analysis scripts produce it automatically. When the codebase changes, re-run the pipeline and the navigation updates.
