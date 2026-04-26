# Hermes Agent -- Data Flow (End-to-End)

## Overview

Three end-to-end flows showing how data moves through the entire Hermes system:
1. Telegram message → Agent → Response
2. Cron job execution
3. Self-improvement (skill creation)

## Flow 1: Telegram Message to Response

A user sends a message to the Hermes bot on Telegram.

```mermaid
sequenceDiagram
    participant User
    participant TG as Telegram API
    participant Adapter as Telegram Adapter
    participant GW as Gateway Runner
    participant Session as Session Manager
    participant Agent as AIAgent
    participant Memory as Memory Manager
    participant Prompt as Prompt Builder
    participant LLM as Anthropic API
    participant Registry as Tool Registry
    participant Terminal as Terminal Tool

    User->>TG: "Check if the tests pass"
    TG->>Adapter: WebSocket event
    Adapter->>GW: on_message(telegram, user_123, chat_456, text)

    GW->>Session: get_or_create(telegram, user_123, chat_456)
    Session->>Session: Load history.jsonl
    Session->>Session: Load MEMORY.md (global + user)
    Session-->>GW: Session ready

    GW->>Agent: run("Check if the tests pass")
    Agent->>Memory: prefetch(messages)
    Memory->>Memory: Read MEMORY.md, USER.md, SOUL.md
    Memory-->>Agent: Memory context

    Agent->>Prompt: build(identity, user, skills, memory, context)
    Prompt-->>Agent: System prompt (2K tokens)

    Agent->>Registry: get_tool_schemas()
    Registry-->>Agent: JSON schemas for active tools

    Agent->>LLM: POST /v1/messages (stream)
    LLM-->>Agent: text: "I'll run the tests for you."
    LLM-->>Agent: tool_call: terminal({ command: "npm test" })

    Agent->>Registry: dispatch("terminal", { command: "npm test" })
    Registry->>Terminal: terminal_handler(params, context)
    Terminal->>Terminal: Execute: npm test
    Terminal-->>Registry: "42 passing, 0 failing"
    Registry-->>Agent: Tool result

    Agent->>LLM: tool_result + continue
    LLM-->>Agent: text: "All 42 tests pass. No failures."

    Agent->>Memory: sync(messages)
    Agent-->>GW: "All 42 tests pass. No failures."

    GW->>Session: Save history.jsonl
    GW->>Adapter: send_message(chat_456, response)
    Adapter->>TG: POST sendMessage (MarkdownV2)
    TG-->>User: "All 42 tests pass. No failures."
```

### What Happens at Each Layer

| Layer | Responsibility |
|-------|---------------|
| Telegram Adapter | Parse WebSocket event, extract user/channel/text |
| Gateway Runner | Route to session, coordinate agent lifecycle |
| Session Manager | Load/save history, manage per-user state |
| Memory Manager | Prefetch relevant memories, sync after response |
| Prompt Builder | Assemble system prompt from persona + memory + context |
| AIAgent | Loop: call LLM, execute tools, repeat until done |
| Tool Registry | Dispatch tool calls to handlers |
| Terminal Tool | Execute shell commands, return output |
| Anthropic API | Generate responses, decide tool calls |

## Flow 2: Cron Job Execution

A scheduled job runs every morning at 9am.

```mermaid
sequenceDiagram
    participant Scheduler as Cron Scheduler
    participant Job as Job: morning-report
    participant Agent as AIAgent
    participant LLM as LLM API
    participant Tools as Web Search + Terminal
    participant Delivery as Message Delivery
    participant TG as Telegram

    Scheduler->>Scheduler: Tick (60s interval)
    Scheduler->>Job: is_due? (9:00 AM, cron: "0 9 * * *")
    Job-->>Scheduler: Yes

    Scheduler->>Agent: Create fresh AIAgent
    Scheduler->>Agent: run("Generate morning report: check CI status, PR reviews needed, deployment health")

    Agent->>LLM: Stream with tools
    LLM-->>Agent: tool_call: web_search({ query: "github.com/org/repo/actions" })
    Agent->>Tools: Execute web_search
    Tools-->>Agent: CI status: 3 passing, 1 failing

    LLM-->>Agent: tool_call: terminal({ command: "gh pr list --repo org/repo" })
    Agent->>Tools: Execute terminal
    Tools-->>Agent: 5 PRs open, 2 need review

    LLM-->>Agent: text: "Morning Report:\n- CI: 3/4 passing (pipeline #127 failing)\n- PRs: 5 open, 2 need your review\n- Deploy: healthy"

    Agent-->>Scheduler: Report text

    Scheduler->>Delivery: deliver(telegram, chat_456, report)
    Delivery->>Delivery: Format as MarkdownV2
    Delivery->>TG: POST sendMessage
    TG-->>Delivery: Delivered

    Scheduler->>Job: Update last_run = 2026-04-26T09:00:00Z
```

### Key Differences from Direct Messages

1. **No user message** -- the job task description replaces the user message
2. **Fresh agent** -- no conversation history from previous runs
3. **Automatic delivery** -- results go to the configured platform/channel
4. **No interaction** -- the agent runs to completion without user input
5. **Isolation** -- each job has its own agent instance and context

## Flow 3: Self-Improvement (Skill Creation)

The agent solves a problem and creates a skill for future use.

```mermaid
sequenceDiagram
    participant User
    participant Agent as AIAgent
    participant LLM
    participant Terminal as Terminal Tool
    participant FileTools as File Tools
    participant Skills as Skills System

    User->>Agent: "Deploy the staging server"
    Agent->>LLM: How to deploy staging?
    LLM-->>Agent: tool_call: terminal({ command: "ssh staging ./deploy.sh" })
    Agent->>Terminal: Execute SSH deploy
    Terminal-->>Agent: Deploy output (success)
    LLM-->>Agent: "Deployed successfully."

    Note over Agent: Agent recognizes this as reusable

    LLM-->>Agent: tool_call: file_tools.write({
        path: "~/.hermes/skills/deploy-staging/deploy-staging.sh",
        content: "#!/bin/bash\nssh staging ./deploy.sh $@"
    })
    Agent->>FileTools: Create skill script

    LLM-->>Agent: tool_call: file_tools.write({
        path: "~/.hermes/skills/deploy-staging/SKILL.md",
        content: "---\nname: deploy-staging\ndescription: Deploy to staging server\n---"
    })
    Agent->>FileTools: Create SKILL.md

    LLM-->>Agent: tool_call: memory.append({
        content: "Created skill: deploy-staging for future deployments"
    })
    Agent->>Skills: Register new skill

    Agent-->>User: "Deployed to staging. I've also created a 'deploy-staging' skill so I can do this faster next time."

    Note over Agent: Next time user asks to deploy...

    User->>Agent: "Deploy staging"
    Agent->>LLM: (system prompt now includes deploy-staging skill)
    LLM-->>Agent: tool_call: execute_skill({ name: "deploy-staging" })
    Agent->>Skills: Run deploy-staging.sh
    Skills-->>Agent: Deploy output
    Agent-->>User: "Done. Staging deployed."
```

### The Learning Loop

```mermaid
flowchart TD
    PROBLEM[User gives problem] --> SOLVE[Agent solves it<br/>using tools]
    SOLVE --> EVALUATE{Is this reusable?}
    EVALUATE -->|Yes| CREATE_SKILL[Create skill<br/>Script + SKILL.md]
    EVALUATE -->|No| DONE[Done]
    CREATE_SKILL --> UPDATE_MEMORY[Update MEMORY.md<br/>Record skill creation]
    UPDATE_MEMORY --> REGISTER[Register skill<br/>Available in future sessions]
    REGISTER --> DONE

    DONE --> NEXT[Next similar problem]
    NEXT --> HAS_SKILL{Has relevant skill?}
    HAS_SKILL -->|Yes| USE_SKILL[Use skill directly<br/>Faster, consistent]
    HAS_SKILL -->|No| SOLVE
    USE_SKILL --> DONE2[Done faster]
```

## Data Formats

### Message Format (OpenAI-compatible)

```json
{
  "role": "user",
  "content": "Check if the tests pass"
}

{
  "role": "assistant",
  "content": "I'll run the tests.",
  "tool_calls": [{
    "id": "tc_1",
    "type": "function",
    "function": {
      "name": "terminal",
      "arguments": "{\"command\": \"npm test\"}"
    }
  }]
}

{
  "role": "tool",
  "tool_call_id": "tc_1",
  "content": "42 passing, 0 failing"
}
```

### Session Storage (JSONL)

```jsonl
{"role":"user","content":"Check if the tests pass","timestamp":"2026-04-26T10:00:00Z","platform":"telegram","user_id":"user_123"}
{"role":"assistant","content":"I'll run the tests.","tool_calls":[...],"timestamp":"2026-04-26T10:00:01Z"}
{"role":"tool","tool_call_id":"tc_1","content":"42 passing, 0 failing","timestamp":"2026-04-26T10:00:05Z"}
{"role":"assistant","content":"All 42 tests pass.","timestamp":"2026-04-26T10:00:06Z"}
```

### Cron Job (JSONL)

```jsonl
{"id":"job_1","name":"morning-report","schedule":"0 9 * * *","task":"Generate morning report...","platform":"telegram","channel":"456","enabled":true,"last_run":"2026-04-26T09:00:00Z"}
```

### Skill Definition

```yaml
# SKILL.md frontmatter
---
name: deploy-staging
description: Deploy to the staging server
parameters:
  branch:
    type: string
    description: Branch to deploy
    default: main
---
```

## Full System Data Flow

```mermaid
flowchart TD
    subgraph "Input"
        TG_IN[Telegram]
        DC_IN[Discord]
        SL_IN[Slack]
        CRON_IN[Cron Scheduler]
        CLI_IN[CLI/TUI]
    end

    subgraph "Session Layer"
        SESSION[Session Manager<br/>Per-user state]
    end

    subgraph "Agent Core"
        AGENT[AIAgent]
        PROMPT[Prompt Builder]
        MEMORY[Memory Manager]
        CONTEXT[Context Engine]
        TOOLS[Tool Registry]
    end

    subgraph "LLM Layer"
        ADAPTER[LLM Adapter]
        LLM_API[Provider API]
    end

    subgraph "Tool Execution"
        TERM[Terminal]
        BROWSER[Browser]
        FILES[Files]
        WEB[Web]
        DELEGATE[Subagents]
        MEM_TOOL[Memory Tool]
        SKILL_EXEC[Skill Runner]
    end

    subgraph "Output"
        TG_OUT[Telegram]
        DC_OUT[Discord]
        SL_OUT[Slack]
        CLI_OUT[CLI/TUI]
    end

    TG_IN & DC_IN & SL_IN --> SESSION
    CRON_IN --> AGENT
    CLI_IN --> AGENT

    SESSION --> AGENT
    AGENT --> PROMPT
    AGENT --> MEMORY
    AGENT --> CONTEXT
    AGENT --> TOOLS

    PROMPT --> AGENT
    MEMORY --> AGENT
    AGENT --> ADAPTER --> LLM_API
    LLM_API --> ADAPTER --> AGENT

    TOOLS --> TERM & BROWSER & FILES & WEB & DELEGATE & MEM_TOOL & SKILL_EXEC
    TERM & BROWSER & FILES & WEB & DELEGATE & MEM_TOOL & SKILL_EXEC --> TOOLS

    AGENT --> SESSION
    SESSION --> TG_OUT & DC_OUT & SL_OUT
    AGENT --> CLI_OUT
```
