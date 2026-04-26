---
title: "Pi Extensions -- pi-design-deck"
---

# pi-design-deck

**Multi-slide visual decision decks with high-fidelity previews.**

Design Deck presents multi-slide visual decision decks when choices need visual comparison. Each slide shows 2-4 high-fidelity previews -- code diffs, architecture diagrams, UI mockups -- and you pick one per slide. The agent gets back a clean selection map and moves on to implementation.

## Usage

Just ask. The agent reaches for the design deck when visual comparison makes sense:

```
show me 3 architecture options for the backend
present a few UI directions for the settings page
what are my options for the auth flow? show me visually
read the PRD at docs/api-plan.md and present the key decisions
```

## Rendering

| Platform | Renderer |
|----------|----------|
| macOS | Glimpse (native WKWebView window) |
| Other | Browser tab fallback |

## Why It Exists

Some decisions are better made visually. Architecture choices, UI directions, and design patterns are hard to compare through text alone. Design Deck gives you side-by-side comparisons with code diffs, diagrams, and mockups rendered at full fidelity.

The agent receives your selections as a clean map -- "slide 1: option B, slide 2: option A" -- and proceeds to implement without ambiguity.

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-design-deck` |
| Trigger | Natural language requests for visual comparison |
| Rendering | Glimpse (macOS) / browser tab (fallback) |
