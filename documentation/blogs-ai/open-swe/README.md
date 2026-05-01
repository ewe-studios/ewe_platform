# Open SWE -- Internal Coding Agent

Open-source framework for internal coding agents. Triggered from Slack, Linear, or GitHub — runs code changes in isolated cloud sandboxes and opens PRs.

## Documents

- [00 Architecture](00-architecture.md) — Deep Agents-based agent with 21 tools, 4 middleware hooks, sandbox providers, webhook server

## Workflow

```
Slack/Linear/GitHub --> Webhook --> LangGraph Run --> Sandbox --> Deep Agent --> PR
```

## Key Features

- **Mid-run message injection** — Linear comments/Slack replies arrive during execution
- **PR safety net** — `open_pr_if_needed` middleware opens PR if agent forgot
- **5 sandbox providers** — LangSmith, Daytona, Runloop, Modal, Local
