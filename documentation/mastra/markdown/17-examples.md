# Mastra -- Examples

## Overview

The `examples/` directory contains standalone projects demonstrating key Mastra APIs that aren't covered by the templates (which focus on end-user agent patterns). These are developer-facing examples showing how to use Mastra programmatically.

**Two examples:**
1. **agent/** — Demonstrates request context presets, MCP client elicitation, Mastra primitive validation, trace seeding, and model middleware
2. **agent-v6/** — Demonstrates structured output with the v6 API

## examples/agent: Comprehensive Feature Demo

### Purpose

The `agent/` example serves as a **kitchen-sink demonstration** of Mastra's core APIs. It includes multiple entry points, each focusing on a different feature:

```
examples/agent/src/
├── index.ts                  ← Mastra primitive validation (get, getById, list)
├── client.ts                 ← MCP client with elicitation (human-in-the-loop prompts)
├── model-middleware.ts       ← Model middleware for logging raw responses
└── mastra/                   ← Mastra instance with agents, workflows, tools
```

### 1. Primitive Validation (`index.ts`)

Validates every Mastra primitive's three access patterns:

```typescript
// Each primitive supports: get(key), getById(id), list()
mastra.getAgent("chefAgent");        // get by key
mastra.getAgentById(agent.id);       // get by ID
mastra.listAgents();                 // list all

// Applies to: Agents, Workflows, Scorers, MCP Servers,
//             Vectors, Tools, Processors
```

This is a diagnostic script that confirms all CRUD operations work correctly across the Mastra instance.

### 2. MCP Client Elicitation (`client.ts`)

Demonstrates the **elicitation pattern** -- when an MCP server requests structured information from the user mid-tool-execution:

```typescript
const mcpClient = new MCPClient({
  servers: {
    myMcpServerTwo: {
      url: new URL('http://localhost:4111/api/mcp/myMcpServerTwo/mcp'),
    },
  },
});

mcpClient.elicitation.onRequest('myMcpServerTwo', elicitationHandler);
```

The elicitation handler prompts the user via readline for each required field, then asks for confirmation before submitting. This is how Mastra implements human-in-the-loop data collection from MCP tools.

### 3. Request Context Presets

Request context presets are named JSON configurations selectable in the Mastra Studio Playground:

```bash
mastra dev --request-context-presets ./request-context-presets.json
```

The example includes presets for:
- **Environments**: development, staging, production
- **Roles**: admin-user, guest-user

The agent dynamically adapts its model selection, tool availability, and instructions based on the active preset.

### 4. Trace Seeding (`seed-traces.mjs`)

A script that generates test data for the observability system:

```typescript
// Step 1: Send 15 prompts to an agent via API
const res = await fetch(`${BASE}/api/agents/simple-assistant/generate`, {...});

// Step 2: Fetch trace IDs from observability API
const traceIds = await fetchTraceIds(runStartTime);

// Step 3: Score each trace with answer-relevancy-scorer
await seedScore(traceId, 'answer-relevancy-scorer', scoreValue, reason);
```

This demonstrates the full eval pipeline: agent call → trace creation → scoring.

### 5. Model Middleware (`model-middleware.ts`)

A `LanguageModelV2Middleware` that intercepts raw model responses for fixture generation:

```typescript
export const logDataMiddleware: LanguageModelV2Middleware = {
  wrapGenerate: async ({ doGenerate, params }) => {
    const result = await doGenerate();
    console.log(JSON.stringify(result, null, 2));
    return result;
  },
  wrapStream: async ({ doStream, params }) => {
    const { stream, ...rest } = await doStream();
    const chunks: LanguageModelV2StreamPart[] = [];
    const transformStream = new TransformStream({
      transform(chunk, controller) {
        chunks.push(chunk);
        controller.enqueue(chunk);
      },
      flush() {
        fs.writeFileSync(`stream-${i}.json`, JSON.stringify(chunks, null, 2));
      },
    });
    return { stream: stream.pipeThrough(transformStream), ...rest };
  },
};
```

This is used to capture raw responses for e2e test fixtures in the `e2e-tests/` directory.

## examples/agent-v6: Structured Output

### Purpose

Demonstrates Mastra's structured output capabilities with the v6 API -- generating typed JSON responses from LLM calls.

```
examples/agent-v6/src/
├── structured-output-example.ts      ← Agent structured output
└── js-client-structured-output-example.ts ← JavaScript client structured output
```

These examples show how to define Zod schemas and have LLMs produce conforming JSON output, both through the Mastra server API and the JavaScript client SDK.

## Related Documents

- [04-tool-system.md](./04-tool-system.md) -- Tool system with MCP integration
- [06-memory-system.md](./06-memory-system.md) -- Memory system
- [07-processors.md](./07-processors.md) -- Processor pipeline
- [14-rl-training-traces.md](./14-rl-training-traces.md) -- Observability and trace seeding

## Source Paths

```
examples/
├── agent/
│   ├── src/
│   │   ├── index.ts                      ← Primitive validation (get, getById, list)
│   │   ├── client.ts                     ← MCP client with elicitation
│   │   └── model-middleware.ts           ← Model response logging middleware
│   ├── seed-traces.mjs                   ← Trace + score seeding script
│   └── request-context-presets.json      ← Named context configurations
└── agent-v6/
    └── src/
        ├── structured-output-example.ts  ← Server-side structured output
        └── js-client-structured-output-example.ts ← Client SDK structured output
```
