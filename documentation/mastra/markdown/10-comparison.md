# Mastra vs Pi vs Hermes -- Comparison

## At a Glance

| Aspect | Pi | Hermes | Mastra |
|--------|-----|--------|--------|
| Language | TypeScript | Python | TypeScript |
| Runtime | Node.js (ESM) | Python 3.11+ | Node.js (ESM) |
| License | Open source | MIT | Apache-2.0 |
| Author | Mario Zechner | Nous Research | Mastra AI |
| Packages | 7 npm packages | Single Python package | pnpm monorepo (20+ packages) |
| Focus | Coding agent (terminal-first) | Multi-platform conversational agent | Framework for building agents |

## Architecture

### Pi

Pi is a **7-package monorepo** with clear layering:

```
pi-ai (foundation: LLM API)
  ↓
pi-agent-core (runtime: agent loop)
  ↓
pi-coding-agent (flagship app)  pi-tui (UI)  pi-mom (Slack)  pi-pods (GPU)  pi-web-ui
```

Each package is independently usable. The coding agent is the flagship application.

### Hermes

Hermes is a **single Python package** with internal module separation:

```
run_agent.py (AIAgent class)
  ├── agent/ (LLM adapters, prompt builder, error classifier)
  ├── tools/ (40+ tools)
  ├── gateway/ (10+ platform adapters)
  ├── memory/ (8+ providers)
  └── plugins/ (memory, context, image gen)
```

Everything ships together. The agent is designed to run standalone on any infrastructure.

### Mastra

Mastra is a **pnpm workspace monorepo** with `@mastra/core` at the center:

```
@mastra/core (agent, loop, LLM, memory, tools, processors)
  ↓
@mastra/memory  @mastra/rag  @mastra/server  @mastra/cli  @mastra/deployer  @mastra/playground
  ↓
integrations/ (provider-specific integrations)
```

It's a framework -- you compose the pieces for your use case. No flagship application.

## Agent Loop

| Aspect | Pi | Hermes | Mastra |
|--------|-----|--------|--------|
| Pattern | Async while-loop | Sync while-loop | Workflow-based stream |
| File | `agent-loop.ts` | `run_agent.py:run_conversation()` | `loop/loop.ts` |
| Pause/resume | No | No | Yes (workflow suspension) |
| Streaming | Native async/await SSE | Sync OpenAI SDK | Workflow stream transform |
| Tool concurrency | `Promise.all()` (unbounded) | `ThreadPoolExecutor` (max 8) | `toolCallConcurrency` param |

**Pi's loop** is a straightforward async loop:

```typescript
// Pi
async function agentLoop() {
  while (true) {
    const response = await llm.stream(messages);
    if (response.toolCalls) {
      await Promise.all(response.toolCalls.map(execute));
    } else break;
  }
}
```

**Hermes's loop** is sync with async wrappers:

```python
# Hermes
def run_conversation():
    while True:
        response = client.chat.completions.create(...)  # Sync SDK
        if response.tool_calls:
            with ThreadPoolExecutor(max_workers=8) as exec:
                results = list(exec.map(execute, tool_calls))
        else: break
```

**Mastra's loop** is workflow-based:

```typescript
// Mastra
function loop({ models, messageList, tools }) {
  return workflowLoopStream({
    // MODEL_STEP → TOOL_STEP → MODEL_STEP → ... → OUTPUT_PROC
  });
}
```

## Tool System

| Aspect | Pi | Hermes | Mastra |
|--------|-----|--------|--------|
| Definition | TypeScript functions | Python functions | `createTool()` with schemas |
| Schema | TypeBox | Custom JSON schema | Zod / JSON Schema / AI SDK |
| Validation | Runtime validation | Manual validation | Schema-based validation |
| Approval | Not built-in | Not built-in | `requireApproval: true` |
| Suspension | Not built-in | Not built-in | `suspend()` / `resume()` |
| Background | `Promise.all()` concurrent | `ThreadPoolExecutor` parallel | Pubsub-based distributed |
| Provider tools | Via AI package | Via adapter | Via router passthrough |

**Mastra's suspension** is unique:

```typescript
const approvalTool = createTool({
  suspendSchema: z.object({ action: z.string() }),
  resumeSchema: z.object({ approved: z.boolean() }),
  execute: async (input, context) => {
    if (!context.isSuspended) {
      context.suspend({ action: input.action });
      return;  // Pauses here, waits for user
    }
    return { approved: context.getResumeData().approved };
  },
});
```

## Model Routing

| Aspect | Pi | Hermes | Mastra |
|--------|-----|--------|--------|
| Approach | AI package provider map | Per-provider adapter class | Model router with gateways |
| Providers | 20+ | 30+ | 200+ |
| Resolution | `getProvider(providerId)` | `get_adapter(model_name)` | `findGatewayForModel("openai/gpt-5")` |
| Fallbacks | Model switching in AI package | Credential pool rotation | Agent-level fallback chain |
| Offline mode | No | No | Yes (`MASTRA_OFFLINE=true`) |
| Gateway plugins | No | No | Yes (Mastra, Netlify, models.dev) |

**Mastra's gateway architecture** is the most flexible:

```
"openai/gpt-5" → findGatewayForModel() → MastraGateway → createOpenAI()
"anthropic/claude" → findGatewayForModel() → ModelsDevGateway → createAnthropic()
"local/llama" → findGatewayForModel() → Custom Gateway → createOpenAI({ baseURL })
```

## Memory

| Aspect | Pi | Hermes | Mastra |
|--------|-----|--------|--------|
| Model | Message history + compaction | Multi-provider (Honcho, Mem0) | Thread-based + semantic recall |
| Working memory | N/A | MemoryManager.get_context() | Template-based, LLM-updated |
| Semantic search | N/A | Provider-dependent | Built-in vector integration |
| Compaction | Token-budget model | 4-phase compression | Token limiting via processors |
| Storage | JSONL files | SQLite/JSON/Provider | Storage ABC (pluggable) |
| Processor integration | N/A | N/A | Memory as input processor |

## Processing Pipeline

| Aspect | Pi | Hermes | Mastra |
|--------|-----|--------|--------|
| Concept | Extensions/skills | Plugins | Processor pipeline |
| Pre-LLM | Prompt templates | Prompt builder | Input processors |
| Post-LLM | Response parsing | Tool result handling | Output processors |
| Error recovery | Retry in loop | Error classifier + retry | Error processors |
| Workflow integration | N/A | N/A | Processors can be workflows |

**Mastra's processor pipeline** is unique -- each processor is a first-class citizen:

```
Input: Memory → Skills → Workspace Instructions → LLM
Output: Structured Output → Custom Formatter → Tool Result Reminder → Client
Error: Prefill Handler → Fallback Handler → Retry/Fail
```

## Background Execution

| Aspect | Pi | Hermes | Mastra |
|--------|-----|--------|--------|
| Model | Concurrent execution | Parallel execution | Distributed execution |
| Mechanism | `Promise.all()` | `ThreadPoolExecutor` | Pubsub + worker processes |
| Task tracking | N/A | N/A | BackgroundTaskManager |
| Status checking | N/A | N/A | Built-in `check_background_task` tool |
| Cancellation | N/A | N/A | AbortController + timeout |

## Sub-Agent Delegation

| Aspect | Pi | Hermes | Mastra |
|--------|-----|--------|--------|
| Support | Not native | Sub-agent patterns | First-class with hooks |
| Message filtering | N/A | Manual | `onMessageFilter` hook |
| Pre-delegation hook | N/A | N/A | `onDelegationStart` |
| Post-delegation hook | N/A | N/A | `onDelegationComplete` |
| Cancellation | N/A | N/A | `bail()` function |

## Strengths

### Pi
- **Simple and focused**: 7 packages, each does one thing well
- **Terminal-first**: Excellent TUI with differential rendering
- **Compaction**: Built-in context management with token budgets
- **Extensions**: Rich extension ecosystem (skills, themes, prompts)
- **Learning resources**: Well-documented with clear code examples

### Hermes
- **Multi-platform**: 10+ messaging platforms out of the box
- **Self-improving**: GEPA-based prompt/skill evolution
- **Memory**: 8+ memory providers with context compression
- **RL training**: Built-in trajectory generation for reinforcement learning
- **Cost tracking**: Detailed token usage and pricing across providers
- **Prompt caching**: Sophisticated Anthropic cache optimization

### Mastra
- **Workflow-based loop**: Pause/resume conversations, suspension points
- **200+ providers**: Model router with gateway plugins
- **Processor pipeline**: Input/output/error transformation chain
- **Background tasks**: Pubsub-based distributed execution
- **Sub-agent delegation**: Rich hooks for message filtering and feedback
- **Type safety**: End-to-end TypeScript generics from Agent to tools to processors

## When to Use Which

| Use Case | Recommendation |
|----------|---------------|
| Interactive terminal coding agent | Pi |
| Multi-platform chatbot (Telegram, Discord, etc.) | Hermes |
| Custom agent with workflow orchestration | Mastra |
| RL training data generation | Hermes |
| Enterprise agent with approval workflows | Mastra |
| Simple, composable LLM API wrapper | Pi |
| Agent with semantic memory search | Mastra |
| Agent that runs on any infrastructure | Hermes |
| Agent with background async tasks | Mastra |
| Agent with context compression | Pi or Hermes |
| Agent with sub-agent delegation | Mastra |

## Shared Concepts

All three frameworks share these fundamental patterns:

1. **Agent class as orchestrator**: Central class managing LLM calls, tools, memory
2. **Tool calling**: LLM can call external tools with validated schemas
3. **Streaming**: Support for streaming LLM responses
4. **Model fallbacks**: Automatic failover to backup models
5. **Observability**: Token tracking, cost estimation, logging
6. **Multi-provider**: Support for OpenAI, Anthropic, Google, local models
7. **Type safety**: TypeScript (Pi, Mastra) or type hints (Hermes)

## Related Documents

- [00-overview.md](./00-overview.md) -- Mastra overview
- [../pi/markdown/00-overview.md](../pi/markdown/00-overview.md) -- Pi overview
- [../hermes/markdown/00-overview.md](../hermes/markdown/00-overview.md) -- Hermes overview
