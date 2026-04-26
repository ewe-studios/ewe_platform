# Build Your Own -- Markdown Processing Pipeline

## What This Section Covers

How to structure markdown files so they render correctly, how Astro processes them, and how to extend the pipeline with custom processing for callouts, cross-references, and metadata-driven navigation.

## Markdown File Structure

Every generated documentation page is a Markdown file with YAML frontmatter. The LLM generates these files. The rendering layer consumes them.

### Module Documentation File

```markdown
---
title: "Authentication Middleware"
description: "JWT validation, session management, and role-based access control"
module_path: "src/middleware/auth.ts"
layer: "middleware"
dependencies: ["database", "config", "crypto-utils"]
dependents: ["api-routes", "websocket-handler"]
tags: ["security", "jwt", "rbac"]
generated_at: "2026-04-26T10:00:00Z"
---

## Purpose

The authentication middleware intercepts every incoming HTTP request, extracts
the JWT from the `Authorization` header, validates it against the signing key,
and attaches the decoded user context to the request object.

## Key Code

The core validation logic lives in a single function:

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

### What this does

1. `jwt.verify` checks the token signature and expiry
2. The decoded `sub` claim is used to look up the full user record
3. If the user doesn't exist (deleted account, etc.), authentication fails
4. The returned `UserContext` is attached to the request for downstream handlers

## How It Connects

```mermaid
flowchart LR
    A[Incoming Request] --> B[Auth Middleware]
    B -->|valid token| C[Route Handler]
    B -->|invalid token| D[401 Response]
    B -.->|reads| E[(Database)]
    B -.->|reads| F[Config]
    C -->|uses| G[UserContext]
`` `

## Dependencies

- **database** -- looks up user records by ID
- **config** -- reads `JWT_SECRET` and token options
- **crypto-utils** -- used for token refresh operations

## Used By

- **api-routes** -- all authenticated API endpoints pass through this middleware
- **websocket-handler** -- WebSocket upgrade requests validate tokens here
```

### Connection Documentation File

```markdown
---
title: "Auth Middleware → Database"
description: "How the authentication layer queries user records during token validation"
from_module: "auth"
to_module: "database"
connection_type: "calls"
generated_at: "2026-04-26T10:00:00Z"
---

## The Connection

The auth middleware calls `db.users.findById()` on every authenticated request
to verify that the token's subject (user ID) still corresponds to an active
user account.

## Why This Matters

Tokens are stateless -- they contain a user ID but no guarantee that the user
still exists. A user could be deleted or deactivated between token issuance and
the next request. This database call is the check against that scenario.

## The Code Path

```typescript
// In auth middleware (src/middleware/auth.ts:32)
const user = await db.users.findById(decoded.sub);

// Calls into database layer (src/db/users.ts:15)
export async function findById(id: string): Promise<User | null> {
  return pool.query('SELECT * FROM users WHERE id = $1', [id])
    .then(r => r.rows[0] || null);
}
`` `

## Performance Consideration

This query runs on every authenticated request. The database layer caches
active user records for 30 seconds to avoid hitting the database on every
single request.
```

## Frontmatter Schema Design

The frontmatter schema is the contract between the LLM layer and the rendering layer. Design it carefully.

### Principles

1. **Machine-readable references** -- `dependencies` and `dependents` are arrays of module slugs, not prose descriptions. This enables the rendering layer to generate navigation links.
2. **Source-code pointers** -- `module_path` is the actual file path in the codebase. This lets the rendered page link back to source.
3. **Generation metadata** -- `generated_at` tracks when the LLM produced this content. Useful for staleness detection.
4. **Typed enums** -- `connection_type` is a fixed set of values. The rendering layer can style different connection types differently.

### Validation

Astro's content collections validate frontmatter at build time via Zod schemas (defined in `content/config.ts`). If the LLM generates invalid frontmatter, the build fails with a clear error. This is your safety net.

## Extending Markdown: Custom Callout Syntax

Standard Markdown has no callout/admonition syntax. You can add it using a remark plugin or by having the LLM generate HTML directly.

### Option A: LLM Generates HTML Callouts

Tell the LLM to emit this HTML in its markdown output:

```html
<div class="callout info">
  <strong>Key insight:</strong> This middleware runs before route matching,
  so it sees every request including static file requests.
</div>
```

Style with CSS:

```css
.callout {
  padding: 1rem 1.25rem;
  border-radius: 0.5rem;
  border-left: 4px solid;
  margin-bottom: 1.5rem;
}
.callout.info {
  background: color-mix(in srgb, var(--accent), transparent 90%);
  border-color: var(--accent);
}
.callout.warn {
  background: color-mix(in srgb, #f59e0b, transparent 90%);
  border-color: #f59e0b;
}
.callout.tip {
  background: color-mix(in srgb, #10b981, transparent 90%);
  border-color: #10b981;
}
```

### Option B: Remark Plugin for Blockquote Callouts

Use the GitHub-style `> [!NOTE]` syntax and process it with a remark plugin:

```markdown
> [!NOTE]
> This middleware runs before route matching.
```

Install and configure:

```bash
npm install remark-github-blockquote-alert
```

```javascript
// astro.config.mjs
import remarkAlert from 'remark-github-blockquote-alert';

export default defineConfig({
  markdown: {
    remarkPlugins: [remarkAlert],
  },
});
```

**Recommendation:** Option A is simpler and gives you full control. The LLM can generate the exact HTML you want. No plugin dependencies.

## Extending Markdown: Source File Links

Have the LLM include file paths and line numbers in code block comments:

```markdown
```typescript
// src/middleware/auth.ts:24-41
export async function validateToken(token: string): Promise<UserContext> {
  // ...
}
`` `
```

Then use CSS or a remark plugin to style the first comment line as a file reference:

```css
.astro-code .line:first-child {
  opacity: 0.6;
  font-style: italic;
}
```

Or render it as a clickable link to your repository:

```javascript
// remark plugin (pseudocode) that converts // path:lines into links
function remarkSourceLinks({ repoUrl }) {
  return (tree) => {
    visit(tree, 'code', (node) => {
      const firstLine = node.value.split('\n')[0];
      const match = firstLine.match(/^\/\/ (.+):(\d+-\d+)$/);
      if (match) {
        const [, path, lines] = match;
        // Add metadata for the rendering layer
        node.data = node.data || {};
        node.data.hProperties = {
          'data-source-path': path,
          'data-source-lines': lines,
          'data-source-url': `${repoUrl}/blob/main/${path}#L${lines.replace('-', '-L')}`,
        };
      }
    });
  };
}
```

## Astro Content Collection Queries

### List All Modules

```astro
---
import { getCollection } from 'astro:content';

const modules = await getCollection('modules');
const byLayer = modules.reduce((acc, mod) => {
  const layer = mod.data.layer;
  (acc[layer] = acc[layer] || []).push(mod);
  return acc;
}, {} as Record<string, typeof modules>);
---

{Object.entries(byLayer).map(([layer, mods]) => (
  <section>
    <h2 class="font-mono text-sm text-[var(--fg-muted)] uppercase">{layer}</h2>
    <ul>
      {mods.map(mod => (
        <li>
          <a href={`/modules/${mod.slug}`}>{mod.data.title}</a>
          <span class="text-[var(--fg-soft)] text-sm"> -- {mod.data.description}</span>
        </li>
      ))}
    </ul>
  </section>
))}
```

### Find Related Modules

```astro
---
const related = await getCollection('modules', (entry) =>
  currentModule.data.dependencies.includes(entry.slug) ||
  currentModule.data.dependents.includes(entry.slug)
);
---
```

### Find Connections For a Module

```astro
---
const connections = await getCollection('connections', (entry) =>
  entry.data.from_module === currentModule.slug ||
  entry.data.to_module === currentModule.slug
);
---
```

## Key Decisions

1. **Frontmatter is the API** -- everything the rendering layer needs to build navigation, cross-references, and metadata displays lives in frontmatter. The prose body is for humans.
2. **The LLM generates valid HTML when Markdown isn't enough** -- callouts, custom blocks, and source links use inline HTML rather than custom Markdown syntax that would need plugins.
3. **Astro validates at build time** -- Zod schemas catch malformed LLM output before it reaches users.
4. **File paths in frontmatter enable source linking** -- the rendering layer can generate "View source" links to your repository.
