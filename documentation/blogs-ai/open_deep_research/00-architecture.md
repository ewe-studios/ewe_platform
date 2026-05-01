---
title: Open Deep Research -- Source Architecture
---

# Open Deep Research -- Source Architecture

## Purpose

Open Deep Research is a configurable deep research agent built with LangGraph. Ranked #6 on the Deep Research Bench Leaderboard (RACE score 0.4344). Performs automated multi-iteration research with parallel processing and generates comprehensive reports.

Source: `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.langchain/open_deep_research/`

## Aha Moments

**Aha: 4-node sequential workflow with parallel subgraphs.** The main graph is: clarify → write_brief → supervisor → final_report. The supervisor spawns parallel researcher subgraphs (up to 5 concurrent), each running its own ReAct loop.

**Aha: Token-limit retry with progressive truncation.** Final report generation and compression both retry up to 3 times, progressively truncating input (10% reduction per retry). This handles context overflow gracefully instead of failing.

**Aha: `override_reducer` pattern for state updates.** Instead of always appending (like typical LangGraph reducers), a custom `override_reducer` allows state values to be completely replaced via `{"type": "override", "value": ...}`.

## Agent Architecture

```
START --> clarify_with_user --> write_research_brief --> research_supervisor --> final_report_generation --> END
                                                              |
                                              +---------------+---------------+
                                              |                               |
                                        supervisor (plan)              supervisor_tools
                                              |                               |
                                   ConductResearch tool --> researcher subgraph (parallel, up to N concurrent)
                                                              |
                                                       compress_research
```

## Main Graph (`src/open_deep_research/deep_researcher.py`)

| Node | Lines | Purpose |
|------|-------|---------|
| `clarify_with_user` | 60-115 | Analyzes user input, optionally asks clarifying questions (controlled by `allow_clarification` config) |
| `write_research_brief` | 118-175 | Transforms user messages into structured research brief using `ResearchQuestion` structured output |
| `supervisor` | 178-223 | Lead researcher planning strategy with 3 tools: `ConductResearch`, `ResearchComplete`, `think_tool` |
| `supervisor_tools` | 225-349 | Executes supervisor tool calls, spawns **parallel researcher subgraphs** (default 5 concurrent) |
| `researcher` | 365-424 | Individual researcher with search tools, MCP tools, and `think_tool` |
| `researcher_tools` | 435-509 | Executes researcher tool calls in parallel, checks for completion or max iterations (default 10) |
| `compress_research` | 511-585 | Compresses findings with retry logic for token limit issues (up to 3 attempts) |
| `final_report_generation` | 607-697 | Synthesizes comprehensive report with token-limit retry |

## State (`src/open_deep_research/state.py:1-97`)

```python
class AgentState(TypedDict):
    messages: Annotated[list[BaseMessage], add_messages]
    supervisor_messages: Annotated[list[BaseMessage], add_messages]
    research_brief: str | None
    raw_notes: list[ResearchNote]
    notes: list[str]
    final_report: str | None
```

Three state schemas: `AgentState`, `SupervisorState`, `ResearcherState` — each with its own message list and output fields.

## Configuration (`src/open_deep_research/configuration.py:1-252`)

| Setting | Default | Purpose |
|---------|---------|---------|
| `max_concurrent_research_units` | 5 | Parallel researchers |
| `max_researcher_iterations` | 6 | ReAct loop iterations per researcher |
| `max_react_tool_calls` | 10 | Max tool calls per researcher |

Four separate model configs: `summarization_model` (gpt-4.1-mini), `research_model` (gpt-4.1), `compression_model` (gpt-4.1), `final_report_model` (gpt-4.1).

## Auth (`src/security/auth.py:1-157`)

LangGraph SDK `Auth` middleware with Supabase JWT verification. Thread/assistant lifecycle hooks enforce ownership on create, filter on read/update/delete/search. Store authorization by namespace.

## Legacy Implementations (`src/legacy/`)

- `graph.py` — Plan-and-execute with human-in-the-loop, sequential section writing
- `multi_agent.py:1-488` — Supervisor-researcher with `Send()` for parallel section research

## Evaluation (`tests/`)

- `run_evaluate.py` — Runs against Deep Research Bench (100 PhD-level research tasks)
- Pre-computed results in `tests/expt_results/` for GPT-5, GPT-4.1, Claude 4 Sonnet

[Back to main index → ../README.md](../README.md)
