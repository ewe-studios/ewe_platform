---
title: "Pi Extensions -- pi-interview-tool"
---

# pi-interview-tool

**Interactive web form for gathering user responses to clarification questions.**

Interview Tool opens an interactive form to gather user responses to clarification questions. On macOS, uses Glimpse to render in a native WKWebView window; falls back to a browser tab on other platforms.

## How It Works

When the agent needs clarification on multiple points, instead of asking questions one at a time in chat, it opens a structured form where you can answer all questions at once. This is faster than back-and-forth conversation and provides structured input the agent can act on.

## Installation

```bash
pi install npm:pi-interview
```

Restart Pi to load the extension.

## Requirements

| Requirement | Version |
|-------------|---------|
| pi-agent | v0.35.0 or later (extensions API) |
| Platform | macOS (Glimpse) / Other (browser fallback) |

## Use Cases

- Gathering requirements for a new feature
- Clarifying ambiguous task descriptions
- Collecting configuration preferences
- Interviewing users about desired behavior

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-interview` |
| Requires | pi-agent >= 0.35.0 |
| Rendering | Glimpse (macOS) / browser tab (fallback) |
