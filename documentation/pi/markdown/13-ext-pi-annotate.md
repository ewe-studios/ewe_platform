---
title: "Pi Extensions -- pi-annotate"
---

# pi-annotate

**Visual annotation for AI. Click elements, capture screenshots, fix code.**

Pi Annotate provides a Figma-like annotation experience directly within the Pi coding agent. It combines an element picker (vanilla JS) with the agent's ability to read selectors, box models, accessibility data, and screenshots -- giving the agent everything it needs to fix UI issues.

## How It Works

1. Run `/annotate` in Pi
2. Click UI elements to annotate them (like Figma comments)
3. Add notes and descriptions
4. Submit -- the agent receives selectors, CSS properties, and screenshots
5. The agent fixes the code

## Commands

```
/annotate    -- Start annotation mode
```

## Why It Exists

UI feedback is notoriously hard to communicate to coding agents through text alone. pi-annotate bridges the gap between visual design tools and code execution, letting designers and developers point at problems rather than describe them.

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-annotate` |
| Platform | macOS |
| Trigger | `/annotate` command |
