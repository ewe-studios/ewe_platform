---
title: "Pi Extensions -- pi-gitlab-duo"
---

# pi-gitlab-duo

**GitLab Duo provider extension for Pi.**

pi-gitlab-duo provides access to GitLab Duo AI models (Claude and GPT) through GitLab's AI Gateway, enabling Pi to use GitLab-hosted models for coding tasks.

## Models

| Model ID | Name | Reasoning |
|----------|------|-----------|
| `claude-opus-4-5-20251101` | Claude Opus 4.5 | Extended thinking |
| `claude-sonnet-4-5-20250929` | Claude Sonnet 4.5 | Extended thinking |
| `claude-haiku-4-5-20251001` | Claude Haiku 4.5 | Extended thinking |
| `gpt-5.1-2025-11-13` | GPT-5.1 | Reasoning |
| `gpt-5-mini-2025-08-07` | GPT-5 Mini | Reasoning |
| `gpt-5-codex` | GPT-5 Codex | Reasoning |

All models support extended thinking/reasoning capabilities.

## Installation

```bash
pi install npm:pi-gitlab-duo
```

## Configuration

Configure your GitLab instance URL and access token through Pi's settings. The extension handles authentication and model routing automatically.

## Why It Exists

Organizations using GitLab Duo want to leverage their existing AI infrastructure. This extension bridges Pi's agent capabilities with GitLab's AI Gateway, providing access to Claude and GPT models through the organization's GitLab subscription.

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-gitlab-duo` |
| Requires | GitLab instance with Duo AI enabled |
| Models | 6 models (Claude + GPT families) |
