# Open Deep Research -- Deep Research Agent

Configurable deep research agent built with LangGraph. Ranked #6 on the Deep Research Bench Leaderboard (RACE score 0.4344).

## Documents

- [00 Architecture](00-architecture.md) — 4-node sequential workflow with parallel researcher subgraphs, token-limit retry, override_reducer pattern

## Agent Architecture

```
clarify_with_user --> write_research_brief --> research_supervisor --> final_report_generation
                                                      |
                                           parallel researcher subgraphs (up to 5)
                                                      |
                                               compress_research
```

## Key Settings

| Setting | Default |
|---------|---------|
| `max_concurrent_research_units` | 5 |
| `max_researcher_iterations` | 6 |
| `max_react_tool_calls` | 10 |
