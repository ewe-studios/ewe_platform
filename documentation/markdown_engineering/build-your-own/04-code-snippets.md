# Build Your Own -- Code Snippets and Contextual Highlighting

## The Problem

Documentation sites often show code in isolation. A function appears in a code block with no indication of where it lives, what calls it, or what it depends on. The reader sees syntax but not context.

The goal: code snippets that are **located** (file path + line numbers), **connected** (links to related modules), and **explained** (prose that says why this code matters, not just what it does).

## Snippet Structure

Every code snippet in the generated documentation should follow this pattern:

```markdown
### Token Validation

The core validation happens in a single function that both verifies
the JWT signature and confirms the user still exists:

```typescript
// src/middleware/auth.ts:24-41
export async function validateToken(token: string): Promise<UserContext> {
  const decoded = jwt.verify(token, config.JWT_SECRET, {
    algorithms: ['HS256'],
    issuer: 'your-app',
  });

  const user = await db.users.findById(decoded.sub);
  if (!user) throw new AuthError('User not found');

  return {
    id: user.id,
    email: user.email,
    roles: user.roles,
  };
}
`` `

This calls into the [database layer](/modules/database) via `findById` and reads
configuration from the [config module](/modules/config). If the user has been
deleted since the token was issued, `AuthError` is thrown and the middleware
returns a 401.
```

### The Three Parts

1. **Lead-in prose** -- explains *why* this code matters in the context of the module
2. **Code block with source reference** -- the actual code with `// file:lines` comment
3. **Follow-up prose** -- connects this snippet to other modules, explains edge cases

## Extracting Snippets from Source

The analysis layer extracts functions, types, and key code sections. The LLM then selects which ones to include and writes the contextual prose around them.

### Symbol Extraction Script

```typescript
// analyze/extract-symbols.ts
import { readFileSync } from 'fs';
import { glob } from 'glob';

interface Symbol {
  name: string;
  kind: 'function' | 'class' | 'type' | 'interface' | 'const' | 'enum';
  file: string;
  startLine: number;
  endLine: number;
  source: string;
  exported: boolean;
  docComment?: string;
}

async function extractSymbols(codebasePath: string): Promise<Symbol[]> {
  const files = await glob('**/*.{ts,tsx,js,jsx,rs,py}', {
    cwd: codebasePath,
    ignore: ['node_modules/**', 'dist/**', 'target/**', '.git/**'],
  });

  const symbols: Symbol[] = [];

  for (const file of files) {
    const content = readFileSync(`${codebasePath}/${file}`, 'utf-8');
    const lines = content.split('\n');

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];

      // TypeScript/JavaScript patterns
      const exportMatch = line.match(
        /^export\s+(async\s+)?(?:function|class|type|interface|const|enum)\s+(\w+)/
      );
      if (exportMatch) {
        const name = exportMatch[2];
        const kind = detectKind(line);
        const endLine = findBlockEnd(lines, i);
        const source = lines.slice(i, endLine + 1).join('\n');
        const docComment = extractDocComment(lines, i);

        symbols.push({
          name,
          kind,
          file,
          startLine: i + 1,
          endLine: endLine + 1,
          source,
          exported: true,
          docComment,
        });
      }

      // Rust patterns
      const rustMatch = line.match(
        /^pub\s+(?:async\s+)?(?:fn|struct|enum|trait|type|const)\s+(\w+)/
      );
      if (rustMatch) {
        const name = rustMatch[1];
        const kind = detectKindRust(line);
        const endLine = findBlockEnd(lines, i);
        const source = lines.slice(i, endLine + 1).join('\n');
        const docComment = extractRustDocComment(lines, i);

        symbols.push({
          name,
          kind,
          file,
          startLine: i + 1,
          endLine: endLine + 1,
          source,
          exported: true,
          docComment,
        });
      }
    }
  }

  return symbols;
}

function detectKind(line: string): Symbol['kind'] {
  if (line.includes('function')) return 'function';
  if (line.includes('class')) return 'class';
  if (line.includes('type')) return 'type';
  if (line.includes('interface')) return 'interface';
  if (line.includes('const')) return 'const';
  if (line.includes('enum')) return 'enum';
  return 'function';
}

function detectKindRust(line: string): Symbol['kind'] {
  if (line.includes(' fn ')) return 'function';
  if (line.includes('struct')) return 'class';
  if (line.includes('enum')) return 'enum';
  if (line.includes('trait')) return 'interface';
  if (line.includes('type')) return 'type';
  if (line.includes('const')) return 'const';
  return 'function';
}

function findBlockEnd(lines: string[], start: number): number {
  let depth = 0;
  for (let i = start; i < lines.length; i++) {
    for (const ch of lines[i]) {
      if (ch === '{') depth++;
      if (ch === '}') depth--;
    }
    if (depth === 0 && i > start) return i;
  }
  return start;
}

function extractDocComment(lines: string[], funcLine: number): string | undefined {
  const commentLines: string[] = [];
  for (let i = funcLine - 1; i >= 0; i--) {
    const trimmed = lines[i].trim();
    if (trimmed.startsWith('/**') || trimmed.startsWith('*') || trimmed.startsWith('//')) {
      commentLines.unshift(trimmed);
    } else {
      break;
    }
  }
  return commentLines.length > 0 ? commentLines.join('\n') : undefined;
}

function extractRustDocComment(lines: string[], funcLine: number): string | undefined {
  const commentLines: string[] = [];
  for (let i = funcLine - 1; i >= 0; i--) {
    const trimmed = lines[i].trim();
    if (trimmed.startsWith('///') || trimmed.startsWith('//!')) {
      commentLines.unshift(trimmed.replace(/^\/\/[\/!]\s?/, ''));
    } else {
      break;
    }
  }
  return commentLines.length > 0 ? commentLines.join('\n') : undefined;
}

// Main
const codebasePath = process.argv[2];
if (!codebasePath) {
  console.error('Usage: npx tsx analyze/extract-symbols.ts /path/to/codebase');
  process.exit(1);
}

const symbols = await extractSymbols(codebasePath);
const output = JSON.stringify(symbols, null, 2);

import { writeFileSync, mkdirSync } from 'fs';
mkdirSync('analyze/output', { recursive: true });
writeFileSync('analyze/output/symbols.json', output);
console.log(`Extracted ${symbols.length} symbols from ${codebasePath}`);
```

### Example Output

```json
[
  {
    "name": "validateToken",
    "kind": "function",
    "file": "src/middleware/auth.ts",
    "startLine": 24,
    "endLine": 41,
    "source": "export async function validateToken(token: string): Promise<UserContext> {\n  ...\n}",
    "exported": true,
    "docComment": "/** Validates a JWT token and returns the associated user context */"
  },
  {
    "name": "findById",
    "kind": "function",
    "file": "src/db/users.ts",
    "startLine": 15,
    "endLine": 18,
    "source": "export async function findById(id: string): Promise<User | null> {\n  ...\n}",
    "exported": true,
    "docComment": null
  }
]
```

## Feeding Snippets to the LLM

The LLM receives extracted symbols alongside dependency information and generates documentation that *selects* the most important snippets to show and wraps them in contextual prose.

### Prompt Template for Module Documentation

```markdown
You are generating documentation for a code module. You will receive:
1. The full source file content
2. Extracted symbols (functions, types, classes) with line numbers
3. Dependency information (what this module imports, what imports it)

## Your Output

Generate a Markdown document with YAML frontmatter. The document must:

1. Start with a "Purpose" section (2-3 sentences, what this module does and why it exists)

2. Include a "Key Code" section with the 2-4 most important code snippets:
   - Select functions/types that define the module's public API or core logic
   - Do NOT include utility functions, re-exports, or boilerplate
   - For each snippet, include:
     a. A heading (### Function/Type Name)
     b. 1-2 sentences explaining WHY this code matters (not what it does -- the code shows what)
     c. The code block with a source reference comment: // file:startLine-endLine
     d. 1-2 sentences after the code connecting it to other modules

3. Include a "How It Connects" section with a Mermaid flowchart showing this module's
   relationship to its dependencies and dependents

4. Include a "Dependencies" section listing each dependency with a one-line description
   of what this module uses it for

## Code Snippet Rules

- Include the ACTUAL source code, not simplified versions
- If a function is longer than 30 lines, show the most important section and use
  "// ... (validation logic)" comments to indicate omitted parts
- Always include the file path and line numbers as the first comment
- Use the correct language tag for the code block (typescript, rust, python, etc.)
- Do not add comments to the code that aren't in the original source
```

## Shiki Configuration for Multi-Language Support

If your codebase uses multiple languages, configure Shiki to handle all of them:

```javascript
// site/astro.config.mjs
export default defineConfig({
  markdown: {
    shikiConfig: {
      themes: {
        light: 'github-light',
        dark: 'github-dark',
      },
      langs: [
        'typescript', 'javascript', 'rust', 'python', 'go',
        'bash', 'json', 'yaml', 'toml', 'sql', 'html', 'css',
        'markdown', 'dockerfile',
      ],
    },
  },
});
```

Astro/Shiki includes most common languages by default, but explicitly listing them ensures they're available and gives you a reference for what's supported.

## Rendering Source References as Links

To turn the `// file:lines` comment into a clickable link to your repository:

```astro
---
// site/src/components/CodeBlock.astro
// Wrap rendered code blocks to add source links
interface Props {
  repoUrl: string;
  branch?: string;
}
const { repoUrl, branch = 'main' } = Astro.props;
---
<div class="code-block-wrapper">
  <slot />
</div>

<script define:vars={{ repoUrl, branch }}>
  document.querySelectorAll('.code-block-wrapper .astro-code').forEach(block => {
    const firstLine = block.querySelector('.line:first-child');
    if (!firstLine) return;

    const text = firstLine.textContent?.trim();
    const match = text?.match(/^\/\/\s*(.+):(\d+)-(\d+)$/);
    if (!match) return;

    const [, path, startLine, endLine] = match;
    const url = `${repoUrl}/blob/${branch}/${path}#L${startLine}-L${endLine}`;

    const link = document.createElement('a');
    link.href = url;
    link.target = '_blank';
    link.rel = 'noopener';
    link.className = 'source-link';
    link.textContent = `${path}:${startLine}-${endLine}`;

    block.parentElement?.insertBefore(link, block);
    firstLine.style.display = 'none';
  });
</script>

<style>
  .source-link {
    display: block;
    font-family: 'JetBrains Mono', monospace;
    font-size: 0.75rem;
    color: var(--fg-soft);
    text-decoration: none;
    padding: 0.5rem 1rem 0;
    background: var(--bg-strong);
    border: 1px solid var(--line);
    border-bottom: none;
    border-radius: 0.5rem 0.5rem 0 0;
  }
  .source-link:hover {
    color: var(--accent);
  }
  .source-link + .astro-code {
    border-top-left-radius: 0;
    border-top-right-radius: 0;
    margin-top: 0;
  }
</style>
```

## Snippet Selection Strategy

Not every function deserves a code block. Guide the LLM to select snippets that maximize understanding:

### Include
- Entry points (main functions, route handlers, middleware hooks)
- Core business logic (the function that *does the thing*)
- Non-obvious patterns (error handling that catches a subtle edge case, a performance optimization)
- Type definitions that define the module's API contract

### Exclude
- Re-exports and barrel files
- Configuration boilerplate
- Utility functions that are self-explanatory from their name
- Test setup and teardown
- Import statements (unless the imports themselves tell a story about dependencies)

### Truncation Strategy for Long Functions

When a function is too long to show in full, include the signature and the critical section:

```typescript
// src/pipeline/transform.ts:45-120
export async function transformBatch(items: RawItem[]): Promise<ProcessedItem[]> {
  const validated = items.filter(item => item.status === 'active');

  // ... (filtering and deduplication: lines 52-78)

  // The core transformation - maps raw fields to the normalized schema
  const transformed = validated.map(item => ({
    id: item.external_id,
    name: normalizeString(item.raw_name),
    category: CATEGORY_MAP[item.type] ?? 'uncategorized',
    score: calculateScore(item.metrics),
    timestamp: new Date(item.created_at).toISOString(),
  }));

  // ... (batch insert with retry: lines 95-118)

  return transformed;
}
```

The `// ... (description)` comments tell the reader what was omitted and give them enough context to decide whether to look at the full source.
