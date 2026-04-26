---
title: "Pi Extensions -- pi-custom-compaction"
---

# pi-custom-compaction

**Swap the model and template Pi uses for context compaction.**

pi-custom-compaction lets you override Pi's default compaction model with a custom choice. Optionally trigger compaction at a specific token count instead of Pi's default threshold.

## How It Works

Once enabled, the extension intercepts every compaction -- whether triggered by Pi's built-in mechanism or by the extension itself -- and uses your configured model and template to generate the summary. If all configured models fail to resolve, it falls back to Pi's built-in compaction silently.

## Configuration

Off by default. Pi's built-in compaction works normally until you enable it.

```bash
pi install npm:pi-custom-compaction
```

Configure the compaction model and token threshold through Pi's extension settings.

## Why It Exists

Pi's default compaction model may not be optimal for all use cases. Some users prefer:
- A cheaper model for cost savings
- A smarter model for better summaries
- Custom token thresholds for specific context window sizes

## Fallback Behavior

If all configured models fail, the extension silently falls back to Pi's built-in compaction. No interruption to the agent's workflow.

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-custom-compaction` |
| Default | Off (Pi's built-in compaction active) |
| Fallback | Pi's built-in compaction |
