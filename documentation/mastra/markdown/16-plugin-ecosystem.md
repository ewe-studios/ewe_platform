# Mastra -- Core Sub-Packages and Plugin Ecosystem

## Overview

The Mastra monorepo's `mastra/` directory is itself a **pnpm workspace** with 21 top-level sub-directories. Beyond the core `packages/` directory (which contains `@mastra/core` and related packages documented in [01-architecture.md](./01-architecture.md)), Mastra organizes its ecosystem into purpose-specific directories: auth providers, browser automation, client SDKs, deployers, observability backends, pubsub, server adapters, storage/vector stores, voice providers, workflow runners, workspace providers, and integrations.

**Key insight:** Mastra follows a **plugin-first architecture** — each provider (auth, voice, store, deployer, etc.) is an independently versioned npm package with its own `package.json`, `tsup.config.ts`, and `vitest.config.ts`. This means you install only the providers you need, avoiding dependency bloat.

## Ecosystem Architecture

```mermaid
flowchart TD
    CORE[@mastra/core<br/>Agent, Tools, Memory, Processors]

    CORE --> AUTH[Auth Providers]
    CORE --> BROWSER[Browser Automation]
    CORE --> CLIENT[Client SDKs]
    CORE --> DEPLOY[Deployers]
    CORE --> OBS[Observability]
    CORE --> PSUB[PubSub]
    CORE --> SRV[Server Adapters]
    CORE --> STORE[Storage/Vector Stores]
    CORE --> VOICE[Voice Providers]
    CORE --> WFLOW[Workflow Runners]
    CORE --> WSPACE[Workspace Providers]
    CORE --> INTEG[Integrations]

    AUTH --> A1[better-auth]
    AUTH --> A2[auth0, clerk, firebase, okta, supabase, workos]

    STORE --> S1[pg, libsql, mysql, duckdb]
    STORE --> S2[pinecone, qdrant, chroma, opensearch]
    STORE --> S3[astra, mongodb, turbopuffer, clickhouse]
    STORE --> S4[dynamodb, elasticsearch, lance, s3vectors]
    STORE --> S5[couchbase, mssql, cloudflare-d1, vectorize, upstash]

    VOICE --> V1[openai, elevenlabs, deepgram, azure]
    VOICE --> V2[google, google-gemini-live-api, playai]
    VOICE --> V3[murf, speechify, sarvam, gladia]
    VOICE --> V4[cloudflare, modelslab, openai-realtime-api]

    DEPLOY --> D1[cloud, vercel, cloudflare, netlify]

    OBS --> O1[langfuse, langsmith, posthog, datadog]
    OBS --> O2[arize, arthur, braintrust, sentry]
    OBS --> O3[mastra, otel-exporter, otel-bridge, laminar]

    SRV --> SV1[express, fastify, hono, koa]

    CLIENT --> C1[client-js, ai-sdk, react]

    BROWSER --> B1[agent-browser, stagehand]

    WFLOW --> W1[inngest]

    PSUB --> P1[google-cloud-pubsub]

    WSPACE --> WS1[docker, vercel, e2b, daytona]
    WSPACE --> WS2[gcs, s3, blaxel, agentfs]
```

## 1. Auth Providers (9 packages)

Mastra supports 9 authentication providers for securing Mastra deployments:

| Package | Type | Use Case |
|---------|------|----------|
| **better-auth** | Self-hosted | Lightweight, TypeScript-first auth |
| **auth0** | SaaS | Enterprise SSO, social login |
| **clerk** | SaaS | User management, sessions |
| **firebase** | SaaS | Firebase Auth integration |
| **okta** | Enterprise | Enterprise SAML/OIDC |
| **supabase** | Self-hosted/SaaS | Supabase Auth (PostgreSQL-based) |
| **workos** | Enterprise | WorkOS SSO, directory sync |
| **cloud** | Internal | Mastra Cloud authentication |
| **studio** | Internal | Mastra Studio authentication |

Each auth provider implements the `AuthProvider` interface:

```typescript
interface AuthProvider {
  authenticate(request: Request): Promise<AuthResult>;
  refreshToken(token: string): Promise<AuthResult>;
  revoke(token: string): Promise<void>;
}
```

**Source:** `mastra/auth/*/src/`

## 2. Browser Automation (3 packages)

| Package | Provider | Purpose |
|---------|----------|---------|
| **agent-browser** | Mastra-native | Mastra's own browser automation with DOM interaction |
| **stagehand** | Browserbase | Browserbase's Stagehand for web automation |
| **_test-utils** | Internal | Browser testing utilities |

These packages enable agents to navigate websites, click elements, extract data, and perform web actions. Used by the `template-browsing-agent` and web automation workflows.

**Source:** `mastra/browser/*/src/`

## 3. Client SDKs (3 packages)

| Package | Target | Purpose |
|---------|--------|---------|
| **client-js** | JavaScript/Node.js | Core JavaScript client for Mastra APIs |
| **ai-sdk** | Vercel AI SDK | Bridge between Mastra and Vercel AI SDK for React apps |
| **react** | React | React hooks and components for Mastra integration |

The client SDKs provide typed access to Mastra's agent, workflow, and memory APIs from frontend applications:

```typescript
// client-js usage
import { MastraClient } from '@mastra/client';
const client = new MastraClient({ baseUrl: 'http://localhost:4111' });
const agent = client.getAgent('my-agent');
const response = await agent.generate({ message: 'Hello' });
```

**Source:** `mastra/client-sdks/*/src/`

## 4. Deployers (4 packages)

| Package | Platform | Deployment Target |
|---------|----------|-------------------|
| **cloud** | Mastra Cloud | Mastra's managed hosting |
| **vercel** | Vercel | Serverless functions + edge runtime |
| **cloudflare** | Cloudflare | Workers + Durable Objects |
| **netlify** | Netlify | Edge functions + serverless |

Each deployer packages the Mastra server for its target platform, handling bundling, environment variables, and runtime-specific optimizations.

**Source:** `mastra/deployers/*/src/`

## 5. Observability (14 packages)

Mastra supports 14 observability backends:

| Package | Type | Features |
|---------|------|----------|
| **langfuse** | LLM Observability | Trace viewing, cost tracking, prompt management |
| **langsmith** | LLM Observability | LangChain's observability platform |
| **posthog** | Product Analytics | User behavior, session replay |
| **datadog** | Infrastructure | APM, logs, metrics |
| **arize** | ML Observability | Model performance, drift detection |
| **arthur** | AI Governance | Compliance, fairness monitoring |
| **braintrust** | LLM Evaluation | Evals, scoring, comparisons |
| **sentry** | Error Tracking | Exception capture, breadcrumbs |
| **mastra** | Native | Mastra's built-in observability |
| **otel-exporter** | OpenTelemetry | Generic OTLP export |
| **otel-bridge** | OpenTelemetry | OTLP ingestion |
| **laminar** | Custom | Specialized tracing |
| **clickhouse-design** | Analytics | ClickHouse-based observability design |
| **_test-utils** | Internal | Test helpers |

Each exporter implements the `TracingExporter` interface:

```typescript
interface TracingExporter {
  exportTracingEvent(event: TracingEvent): Promise<void>;
  flush(): Promise<void>;
  shutdown(): Promise<void>;
}
```

**Source:** `mastra/observability/*/src/`

## 6. Server Adapters (5 packages)

| Package | Framework | Purpose |
|---------|-----------|---------|
| **express** | Express.js | Express HTTP server adapter |
| **fastify** | Fastify | Fastify HTTP server (high performance) |
| **hono** | Hono | Edge-compatible HTTP server |
| **koa** | Koa | Koa.js HTTP server |
| **_test-utils** | Internal | Testing helpers |

Each adapter wraps Mastra's API routes in the framework's routing system:

```typescript
// Express adapter
import { createMastraExpressAdapter } from '@mastra/server-express';
const app = express();
app.use('/api/mastra', createMastraExpressAdapter({ mastra }));
```

**Source:** `mastra/server-adapters/*/src/`

## 7. Storage/Vector Stores (22 packages)

The largest plugin category — 22 storage providers spanning relational, vector, and document databases:

### Vector Stores (12)
| Package | Type | Use Case |
|---------|------|----------|
| **pinecone** | Managed vector | Production semantic search |
| **qdrant** | Managed/self-hosted | High-performance vector search |
| **chroma** | Local/embedded | Development, small-scale |
| **opensearch** | Managed | AWS OpenSearch |
| **astra** | Managed | DataStax Astra DB |
| **turbopuffer** | Managed | Fast vector search |
| **lance** | Local/embedded | LanceDB embedded |
| **s3vectors** | AWS | S3-integrated vector search |
| **cloudflare-d1** | Edge | Cloudflare D1 with FTS5 |
| **vectorize** | Managed | Cloudflare Vectorize |
| **upstash** | Serverless | Upstash Vector |
| **clickhouse** | OLAP | ClickHouse vector capabilities |

### Document/Relational Stores (10)
| Package | Type | Use Case |
|---------|------|----------|
| **pg** | PostgreSQL | Primary relational store |
| **libsql** | SQLite/Turso | Edge SQLite |
| **mongodb** | Document | MongoDB Atlas |
| **duckdb** | OLAP | Analytics queries |
| **dynamodb** | NoSQL | AWS DynamoDB |
| **elasticsearch** | Search | Full-text search |
| **couchbase** | NoSQL | Couchbase Server |
| **mssql** | Relational | SQL Server |
| **cloudflare** | Edge | Cloudflare KV/DO |
| **_test-utils** | Internal | Test helpers |

Each store implements the `Store` and/or `VectorStore` ABC:

```typescript
interface VectorStore {
  query(vector: number[], topK: number, filter?: Filter): Promise<ScoredVector[]>;
  upsert(vectors: VectorRecord[], namespace?: string): Promise<void>;
  delete(ids: string[], namespace?: string): Promise<void>;
}

interface Store {
  batchWrite<T extends Record<string, unknown>>(tableName: string, records: T[]): Promise<void>;
  batchRead<T extends Record<string, unknown>>(tableName: string, keys: string[]): Promise<T[]>;
  batchDelete(tableName: string, keys: string[]): Promise<void>;
}
```

**Source:** `mastra/stores/*/src/`

## 8. Voice Providers (14 packages)

Mastra supports 14 voice providers for text-to-speech, speech-to-text, and real-time voice conversations:

| Package | Capabilities | Type |
|---------|-------------|------|
| **openai** | TTS, STT | OpenAI voices |
| **elevenlabs** | TTS (high quality) | ElevenLabs |
| **deepgram** | STT (transcription) | Deepgram |
| **azure** | TTS, STT | Azure Cognitive Services |
| **google** | TTS, STT | Google Cloud Speech |
| **google-gemini-live-api** | Real-time voice | Gemini live conversation |
| **playai** | TTS | PlayAI voices |
| **murf** | TTS | Murf AI voices |
| **speechify** | TTS | Speechify |
| **sarvam** | TTS, STT | Sarvam AI (Indian languages) |
| **gladia** | STT | Gladia transcription |
| **cloudflare** | TTS | Cloudflare AI voices |
| **modelslab** | TTS | ModelsLab voices |
| **openai-realtime-api** | Real-time voice | OpenAI Realtime API |

Each voice provider implements the `VoiceProvider` interface:

```typescript
interface VoiceProvider {
  speak(text: string, options?: SpeakOptions): Promise<AudioStream>;
  listen(audio: AudioStream, options?: ListenOptions): Promise<Transcription>;
  // Real-time providers also support:
  connect(session: VoiceSession): Promise<void>;
}
```

**Source:** `mastra/voice/*/src/`

## 9. Workflow Runners (3 packages)

| Package | Platform | Purpose |
|---------|----------|---------|
| **inngest** | Inngest | Serverless workflow execution with step persistence |
| **_test-utils** | Internal | Workflow testing helpers |
| **README.md** | Documentation | Workflow architecture docs |

Inngest integration enables Mastra workflows to run as serverless functions with automatic retry, scheduling, and step persistence:

```typescript
import { InngestWorkflowRunner } from '@mastra/workflow-inngest';
const runner = new InngestWorkflowRunner({ client: inngestClient });
```

**Source:** `mastra/workflows/inngest/src/`

## 10. PubSub (2 packages)

| Package | Platform | Purpose |
|---------|----------|---------|
| **google-cloud-pubsub** | GCP | Google Cloud Pub/Sub for distributed message passing |

Used for background task coordination and event-driven agent communication:

```typescript
import { GoogleCloudPubSub } from '@mastra/pubsub-google-cloud-pubsub';
const pubsub = new GoogleCloudPubSub({ projectId: 'my-project' });
await pubsub.publish('agent-events', { type: 'task-complete', result });
```

**Source:** `mastra/pubsub/google-cloud-pubsub/src/`

## 11. Workspace Providers (8 packages)

Workspace providers enable Mastra to run in various execution environments:

| Package | Platform | Purpose |
|---------|----------|---------|
| **docker** | Docker | Local Docker container execution |
| **vercel** | Vercel | Vercel serverless execution |
| **e2b** | E2B | E2B sandbox execution |
| **daytona** | Daytona | Daytona workspace execution |
| **gcs** | Google Cloud | Google Cloud Storage workspace |
| **s3** | AWS | S3-based workspace |
| **blaxel** | Blaxel | Blaxel workspace execution |
| **agentfs** | Custom | Agent filesystem workspace |

Each provider implements the `WorkspaceProvider` interface for isolated agent execution:

```typescript
interface WorkspaceProvider {
  create(config: WorkspaceConfig): Promise<Workspace>;
  get(id: string): Promise<Workspace>;
  delete(id: string): Promise<void>;
}
```

**Source:** `mastra/workspaces/*/src/`

## 12. Integrations (1 package)

| Package | Target | Purpose |
|---------|--------|---------|
| **opencode** | OpenCode | Integration with OpenCode editor/IDE |

**Source:** `mastra/integrations/opencode/src/`

## 13. MastraCode (1 package)

**MastraCode** is Mastra's built-in coding agent — a CLI-based code assistant that provides file editing, code analysis, and terminal interaction:

| Component | Purpose |
|-----------|---------|
| `src/agents/memory.ts` | Coding agent memory management |
| `src/tui/event-dispatch.ts` | Terminal UI event system |
| `src/tui/render-messages.ts` | Terminal message rendering |
| `src/ipc/ipc-reporter.ts` | IPC reporting for inter-process communication |
| `src/utils/errors.ts` | Error classification and handling |
| `scripts/index-messages.ts` | Message indexing for training |

**Source:** `mastra/mastracode/src/`, `mastra/mastracode/scripts/`

## 14. Explorations (3 items)

Experimental/prototype code that hasn't graduated to production:

| Exploration | Purpose |
|-------------|---------|
| **ralph-wiggum-loop-prototype** | Prototype agent loop implementation |
| **longmemeval** | LongMemEval benchmark for observational memory testing |
| **network-validation-bridge** | Network validation utilities |

## 15. Enterprise Edition (ee/)

The `ee/` directory currently contains only a `LICENSE` file, indicating Mastra's enterprise features are planned but not yet implemented in this version of the codebase.

## Package Versioning

Each package is independently versioned via pnpm workspaces:

```
@mastra/core        → 1.x.x
@mastra/memory      → 1.6.x
@mastra/libsql      → 1.6.3
@mastra/mcp         → 1.0.3
@mastra/evals       → latest
@mastra/rag         → latest
```

## Comparison: Plugin Coverage

| Category | Mastra | Pi | Hermes |
|----------|--------|----|--------|
| Auth providers | 9 | 0 | 0 |
| Vector stores | 12 | 2 | 1 |
| Voice providers | 14 | 0 | 0 |
| Observability backends | 14 | 0 | 0 |
| Deployers | 4 | 0 | 0 |
| Server adapters | 4 | 0 | 0 |
| Browser automation | 2 | 0 | 0 |
| Workflow runners | 1 | 0 | 0 |
| PubSub providers | 1 | 0 | 0 |
| Workspace providers | 8 | 0 | 0 |

Mastra's plugin ecosystem is **significantly larger** than Pi or Hermes, reflecting its design as a comprehensive framework rather than a focused agent runtime.

## Related Documents

- [01-architecture.md](./01-architecture.md) — Core package map and dependency graph
- [04-tool-system.md](./04-tool-system.md) — Tool system with store integrations
- [06-memory-system.md](./06-memory-system.md) — Memory with vector store backends
- [15-ecosystem.md](./15-ecosystem.md) — Top-level projects outside mastra/

## Source Paths

```
mastra/
├── auth/                    ← 9 auth providers (better-auth, auth0, clerk, etc.)
├── browser/                 ← Browser automation (agent-browser, stagehand)
├── client-sdks/             ← Client SDKs (client-js, ai-sdk, react)
├── deployers/               ← Deploy targets (cloud, vercel, cloudflare, netlify)
├── observability/           ← 14 observability backends (langfuse, posthog, etc.)
├── pubsub/                  ← Google Cloud Pub/Sub integration
├── server-adapters/         ← HTTP servers (express, fastify, hono, koa)
├── stores/                  ← 22 storage providers (pg, pinecone, qdrant, etc.)
├── voice/                   ← 14 voice providers (openai, elevenlabs, etc.)
├── workflows/               ← Workflow runners (inngest)
├── workspaces/              ← 8 workspace providers (docker, vercel, e2b, etc.)
├── integrations/            ← OpenCode integration
├── mastracode/              ← MastraCode coding agent
├── explorations/            ← Experimental prototypes (longmemeval, etc.)
├── ee/                      ← Enterprise edition (placeholder)
├── packages/                ← Core packages (documented in 01-architecture.md)
├── docs/                    ← Documentation tooling
├── templates/               ← Template tooling
├── examples/                ← Example projects
├── patches/                 ← npm patches
└── scripts/                 ← Build and maintenance scripts
```
