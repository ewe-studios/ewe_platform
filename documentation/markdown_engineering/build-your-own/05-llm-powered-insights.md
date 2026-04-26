# Build Your Own -- LLM-Powered Documentation Generation

## The Core Idea

An LLM reads your codebase's structure, source code, dependency graph, and test coverage, then generates documentation that a human would take days to write. The LLM doesn't just describe code -- it explains *why* the code is structured this way, *how* modules connect, and *what* a new developer needs to understand first.

## The Generation Script

This is the central script that ties analysis output to LLM API calls and produces Markdown files.

```typescript
// generate/generate-docs.ts
import Anthropic from '@anthropic-ai/sdk';
import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { join } from 'path';

const client = new Anthropic();

// Load analysis output
const structure = JSON.parse(readFileSync('analyze/output/structure.json', 'utf-8'));
const dependencies = JSON.parse(readFileSync('analyze/output/dependencies.json', 'utf-8'));
const symbols = JSON.parse(readFileSync('analyze/output/symbols.json', 'utf-8'));

// Load prompt templates
const modulePrompt = readFileSync('generate/prompts/module-doc.md', 'utf-8');
const connectionPrompt = readFileSync('generate/prompts/connection-doc.md', 'utf-8');
const overviewPrompt = readFileSync('generate/prompts/overview-doc.md', 'utf-8');

// Output directory (Astro content collections)
const contentDir = 'site/src/content';
mkdirSync(join(contentDir, 'modules'), { recursive: true });
mkdirSync(join(contentDir, 'connections'), { recursive: true });
mkdirSync(join(contentDir, 'overviews'), { recursive: true });

interface Module {
  name: string;
  path: string;
  files: string[];
  layer: string;
}

interface Dependency {
  from: string;
  to: string;
  type: 'imports' | 'calls' | 'implements' | 'extends';
}

// ──────────────────────────────────────────────
// Step 1: Generate documentation for each module
// ──────────────────────────────────────────────

async function generateModuleDoc(mod: Module): Promise<string> {
  // Gather all source files for this module
  const sourceContents = mod.files.map(file => {
    const content = readFileSync(file, 'utf-8');
    return `### ${file}\n\`\`\`\n${content}\n\`\`\``;
  }).join('\n\n');

  // Gather symbols for this module
  const moduleSymbols = symbols.filter((s: any) =>
    mod.files.some((f: string) => s.file === f || f.endsWith(s.file))
  );

  // Gather dependencies
  const deps = dependencies.filter((d: Dependency) => d.from === mod.name);
  const dependents = dependencies.filter((d: Dependency) => d.to === mod.name);

  const prompt = modulePrompt
    .replace('{{MODULE_NAME}}', mod.name)
    .replace('{{MODULE_PATH}}', mod.path)
    .replace('{{MODULE_LAYER}}', mod.layer)
    .replace('{{SOURCE_FILES}}', sourceContents)
    .replace('{{SYMBOLS}}', JSON.stringify(moduleSymbols, null, 2))
    .replace('{{DEPENDENCIES}}', JSON.stringify(deps, null, 2))
    .replace('{{DEPENDENTS}}', JSON.stringify(dependents, null, 2))
    .replace('{{DEP_NAMES}}', JSON.stringify(deps.map((d: Dependency) => d.to)))
    .replace('{{DEPENDENT_NAMES}}', JSON.stringify(dependents.map((d: Dependency) => d.from)));

  const response = await client.messages.create({
    model: 'claude-sonnet-4-6',
    max_tokens: 4096,
    messages: [{ role: 'user', content: prompt }],
  });

  const text = response.content[0];
  if (text.type !== 'text') throw new Error('Expected text response');
  return text.text;
}

// ──────────────────────────────────────────────
// Step 2: Generate connection documentation
// ──────────────────────────────────────────────

async function generateConnectionDoc(
  from: Module,
  to: Module,
  connectionType: string
): Promise<string> {
  // Get source for both modules
  const fromSource = from.files.map(f => readFileSync(f, 'utf-8')).join('\n');
  const toSource = to.files.map(f => readFileSync(f, 'utf-8')).join('\n');

  const prompt = connectionPrompt
    .replace('{{FROM_MODULE}}', from.name)
    .replace('{{TO_MODULE}}', to.name)
    .replace('{{CONNECTION_TYPE}}', connectionType)
    .replace('{{FROM_SOURCE}}', fromSource)
    .replace('{{TO_SOURCE}}', toSource);

  const response = await client.messages.create({
    model: 'claude-sonnet-4-6',
    max_tokens: 2048,
    messages: [{ role: 'user', content: prompt }],
  });

  const text = response.content[0];
  if (text.type !== 'text') throw new Error('Expected text response');
  return text.text;
}

// ──────────────────────────────────────────────
// Step 3: Generate architecture overview
// ──────────────────────────────────────────────

async function generateOverview(modules: Module[]): Promise<string> {
  const moduleList = modules.map(m =>
    `- **${m.name}** (${m.layer}): ${m.files.length} files at ${m.path}`
  ).join('\n');

  const depList = dependencies.map((d: Dependency) =>
    `- ${d.from} ${d.type} ${d.to}`
  ).join('\n');

  const prompt = overviewPrompt
    .replace('{{MODULES}}', moduleList)
    .replace('{{DEPENDENCIES}}', depList)
    .replace('{{MODULE_COUNT}}', String(modules.length))
    .replace('{{FILE_COUNT}}', String(modules.reduce((sum, m) => sum + m.files.length, 0)));

  const response = await client.messages.create({
    model: 'claude-sonnet-4-6',
    max_tokens: 4096,
    messages: [{ role: 'user', content: prompt }],
  });

  const text = response.content[0];
  if (text.type !== 'text') throw new Error('Expected text response');
  return text.text;
}

// ──────────────────────────────────────────────
// Main execution
// ──────────────────────────────────────────────

async function main() {
  const modules: Module[] = structure.modules;
  const timestamp = new Date().toISOString();

  console.log(`Generating documentation for ${modules.length} modules...`);

  // Generate module docs (with concurrency limit)
  const CONCURRENCY = 3;
  for (let i = 0; i < modules.length; i += CONCURRENCY) {
    const batch = modules.slice(i, i + CONCURRENCY);
    const results = await Promise.all(batch.map(generateModuleDoc));

    for (let j = 0; j < batch.length; j++) {
      const slug = batch[j].name.toLowerCase().replace(/[^a-z0-9]+/g, '-');
      writeFileSync(join(contentDir, 'modules', `${slug}.md`), results[j]);
      console.log(`  ✓ ${batch[j].name}`);
    }
  }

  // Generate connection docs for significant connections
  const significantConnections = dependencies.filter((d: Dependency) =>
    d.type === 'imports' || d.type === 'calls'
  );

  console.log(`\nGenerating ${significantConnections.length} connection docs...`);

  for (let i = 0; i < significantConnections.length; i += CONCURRENCY) {
    const batch = significantConnections.slice(i, i + CONCURRENCY);
    const results = await Promise.all(batch.map((conn: Dependency) => {
      const from = modules.find(m => m.name === conn.from)!;
      const to = modules.find(m => m.name === conn.to)!;
      return generateConnectionDoc(from, to, conn.type);
    }));

    for (let j = 0; j < batch.length; j++) {
      const slug = `${batch[j].from}-to-${batch[j].to}`.toLowerCase().replace(/[^a-z0-9]+/g, '-');
      writeFileSync(join(contentDir, 'connections', `${slug}.md`), results[j]);
      console.log(`  ✓ ${batch[j].from} → ${batch[j].to}`);
    }
  }

  // Generate architecture overview
  console.log('\nGenerating architecture overview...');
  const overview = await generateOverview(modules);
  writeFileSync(join(contentDir, 'overviews', 'architecture.md'), overview);
  console.log('  ✓ Architecture overview');

  console.log('\nDone.');
}

main().catch(console.error);
```

## Prompt Templates

### Module Documentation Prompt

```markdown
<!-- generate/prompts/module-doc.md -->
You are generating documentation for the **{{MODULE_NAME}}** module.

## Module Information

- **Path:** {{MODULE_PATH}}
- **Layer:** {{MODULE_LAYER}}
- **Dependencies:** {{DEP_NAMES}}
- **Used by:** {{DEPENDENT_NAMES}}

## Source Files

{{SOURCE_FILES}}

## Extracted Symbols

{{SYMBOLS}}

## Dependencies

{{DEPENDENCIES}}

## Dependents

{{DEPENDENTS}}

## Your Task

Generate a complete Markdown document with this exact structure:

### Frontmatter (YAML)

```yaml
---
title: "<human-readable module name>"
description: "<one sentence describing what this module does>"
module_path: "{{MODULE_PATH}}"
layer: "{{MODULE_LAYER}}"
dependencies: {{DEP_NAMES}}
dependents: {{DEPENDENT_NAMES}}
tags: ["<2-4 relevant tags>"]
generated_at: "<current ISO timestamp>"
---
```

### Body Sections

1. **## Purpose** -- 2-3 sentences. What does this module do? Why does it exist as a
   separate module rather than being inlined elsewhere?

2. **## Key Code** -- Show the 2-4 most important code snippets. For each:
   - A `### Function/Type Name` heading
   - 1-2 sentences before the code block explaining WHY this is important
   - The code block with `// file:startLine-endLine` as the first line
   - 1-2 sentences after linking to related modules using markdown links:
     `[module-name](/modules/module-slug)`

3. **## How It Connects** -- A single Mermaid flowchart showing this module's
   relationships. Use `flowchart TD`. Max 10 nodes.

4. **## Dependencies** -- Bulleted list. For each dependency:
   `- **[name](/modules/slug)** -- what this module uses it for`

5. **## Used By** -- Same format as Dependencies but for dependents.

## Rules

- Write for a developer who is new to this codebase but experienced with the language
- Explain WHY, not WHAT -- the code shows what it does
- Include actual source code, not simplified versions
- If a function is over 30 lines, show the key section and use
  `// ... (description)` for omitted parts
- Use the correct language identifier for code blocks
- Mermaid diagrams: use short labels, no HTML in labels, max 10 nodes
- Cross-reference other modules using markdown links: [name](/modules/slug)
- Do not invent code that isn't in the source
- Do not add comments to source code that aren't in the original
```

### Connection Documentation Prompt

```markdown
<!-- generate/prompts/connection-doc.md -->
You are documenting the connection between **{{FROM_MODULE}}** and **{{TO_MODULE}}**.

Connection type: **{{CONNECTION_TYPE}}**

## Source Code: {{FROM_MODULE}}

```
{{FROM_SOURCE}}
```

## Source Code: {{TO_MODULE}}

```
{{TO_SOURCE}}
```

## Your Task

Generate a Markdown document with this structure:

### Frontmatter

```yaml
---
title: "{{FROM_MODULE}} → {{TO_MODULE}}"
description: "<one sentence describing this connection>"
from_module: "{{FROM_MODULE}}"
to_module: "{{TO_MODULE}}"
connection_type: "{{CONNECTION_TYPE}}"
generated_at: "<current ISO timestamp>"
---
```

### Body

1. **## The Connection** -- 2-3 sentences. What does {{FROM_MODULE}} need from
   {{TO_MODULE}}? Why can't it do this itself?

2. **## The Code Path** -- Show the specific function calls that cross the module
   boundary. Include the calling code in {{FROM_MODULE}} and the called code in
   {{TO_MODULE}}, each with file:line references.

3. **## Data Flow** -- A Mermaid sequence diagram showing the data that flows
   across this connection. Include parameter types and return types.

4. **## Why This Matters** -- 1-2 sentences on what would break if this
   connection were severed.
```

### Architecture Overview Prompt

```markdown
<!-- generate/prompts/overview-doc.md -->
You are generating an architecture overview for a codebase with
{{MODULE_COUNT}} modules across {{FILE_COUNT}} files.

## Modules

{{MODULES}}

## Dependencies Between Modules

{{DEPENDENCIES}}

## Your Task

Generate a Markdown document with this structure:

### Frontmatter

```yaml
---
title: "Architecture Overview"
description: "High-level architecture of the codebase"
generated_at: "<current ISO timestamp>"
---
```

### Body

1. **## System Overview** -- 3-5 sentences describing what this codebase does
   and how it's organized. Written for someone who has never seen the code.

2. **## Architecture Diagram** -- A Mermaid flowchart showing ALL modules and
   their primary dependencies. Group by layer. Use subgraphs for layers.

3. **## Layers** -- For each layer, a paragraph explaining its responsibility
   and listing the modules in it.

4. **## Key Flows** -- Describe the 2-3 most important request/data flows
   through the system. Each flow gets a Mermaid sequence diagram.

5. **## Entry Points** -- Where does execution start? What are the main
   entry points a developer should know about?

6. **## Module Index** -- A table of all modules with columns:
   Module | Layer | Dependencies | Description
```

## Prompt Engineering Principles

### 1. Structured Output > Free-form

The prompts define exact section headings and content requirements. This produces consistent output that the rendering layer can rely on.

### 2. Source Code as Ground Truth

The prompts include actual source code, not descriptions. The LLM reads the code and generates documentation from it. This prevents hallucination about what the code does.

### 3. WHY Over WHAT

Every prompt reinforces: explain why the code is structured this way, not what each line does. Code is self-documenting for WHAT; documentation should cover WHY.

### 4. Cross-references as Links

The prompts tell the LLM to use markdown links (`[name](/modules/slug)`) for cross-references. This creates a navigable web of documentation.

### 5. Diagram Constraints

The prompts cap Mermaid diagrams at 10-15 nodes and forbid HTML in labels. This prevents unreadable diagrams and Mermaid rendering failures.

## API Usage Optimization

### Prompt Caching

When generating docs for many modules, the system prompt and prompt template are repeated. Use Anthropic's prompt caching to avoid re-processing them:

```typescript
// Cache the system prompt across calls
const systemPrompt = readFileSync('generate/prompts/system.md', 'utf-8');

async function generateWithCaching(userPrompt: string): Promise<string> {
  const response = await client.messages.create({
    model: 'claude-sonnet-4-6',
    max_tokens: 4096,
    system: [{
      type: 'text',
      text: systemPrompt,
      cache_control: { type: 'ephemeral' },
    }],
    messages: [{ role: 'user', content: userPrompt }],
  });

  const text = response.content[0];
  if (text.type !== 'text') throw new Error('Expected text response');
  return text.text;
}
```

With prompt caching, the base system prompt is processed once and reused across all module documentation calls. This reduces both latency and cost.

### Batching for Large Codebases

For codebases with 100+ modules, use the Anthropic Batch API:

```typescript
import Anthropic from '@anthropic-ai/sdk';

const client = new Anthropic();

// Create batch requests
const requests = modules.map((mod, i) => ({
  custom_id: `module-${mod.name}`,
  params: {
    model: 'claude-sonnet-4-6',
    max_tokens: 4096,
    messages: [{ role: 'user', content: buildPrompt(mod) }],
  },
}));

// Submit batch
const batch = await client.batches.create({ requests });

// Poll for completion
let result = await client.batches.retrieve(batch.id);
while (result.processing_status === 'in_progress') {
  await new Promise(resolve => setTimeout(resolve, 30000));
  result = await client.batches.retrieve(batch.id);
}

// Retrieve results
const results = [];
for await (const item of client.batches.results(batch.id)) {
  results.push(item);
}
```

Batch API gives 50% cost reduction and handles rate limiting automatically.

### Concurrency Control

The generation script limits concurrent API calls to 3. Adjust based on your rate limits:

```typescript
const CONCURRENCY = 3;

for (let i = 0; i < modules.length; i += CONCURRENCY) {
  const batch = modules.slice(i, i + CONCURRENCY);
  await Promise.all(batch.map(generateModuleDoc));
}
```

## Cost Estimates

| Codebase Size | Modules | API Calls | Estimated Cost (Sonnet) |
|--------------|---------|-----------|------------------------|
| Small (20 files) | 5-10 | ~15 | ~$0.50 |
| Medium (100 files) | 20-40 | ~60 | ~$3.00 |
| Large (500 files) | 50-100 | ~150 | ~$8.00 |
| Very large (2000+ files) | 100-200 | ~300 (use Batch API) | ~$10.00 |

These estimates assume Claude Sonnet 4.6 pricing. Using Haiku for simpler modules and Sonnet for complex ones can reduce costs further.

## Incremental Regeneration

Don't regenerate everything when one file changes. Track what's stale:

```typescript
import { statSync, existsSync } from 'fs';

function isStale(mod: Module, outputPath: string): boolean {
  if (!existsSync(outputPath)) return true;

  const outputMtime = statSync(outputPath).mtimeMs;

  return mod.files.some(file => {
    const sourceMtime = statSync(file).mtimeMs;
    return sourceMtime > outputMtime;
  });
}

// Only regenerate stale modules
const staleModules = modules.filter(mod => {
  const slug = mod.name.toLowerCase().replace(/[^a-z0-9]+/g, '-');
  const outputPath = join(contentDir, 'modules', `${slug}.md`);
  return isStale(mod, outputPath);
});

console.log(`${staleModules.length} of ${modules.length} modules are stale`);
```

This compares source file modification times against generated documentation modification times. Only stale modules trigger LLM calls.
