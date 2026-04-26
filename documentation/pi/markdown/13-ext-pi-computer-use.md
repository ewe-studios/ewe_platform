---
title: "Pi Extensions -- pi-computer-use"
---

# pi-computer-use

**Codex-style computer use for Pi on macOS with AX-first semantic targeting.**

pi-computer-use gives Pi the ability to interact with the computer's GUI -- clicking elements, typing text, and waiting for UI changes. It uses an accessibility-tree-first approach for semantic targeting, falling back to image-based coordination only when semantic coverage is weak.

## Features

| Tool | Description |
|------|-------------|
| `screenshot` | Capture the current screen |
| `click` | Click by coordinates or semantic reference (`@eN`) |
| `type_text` | Type text into focused element |
| `wait` | Wait for UI changes |

## AX-First Semantic Targeting

Instead of clicking by pixel coordinates (which breaks when UI layout changes), pi-computer-use uses the macOS Accessibility (AX) tree to target elements by semantic identity:

```typescript
click({ ref: "@e1" })  // Semantic targeting -- resilient to layout changes
click({ x: 100, y: 200 })  // Coordinate click -- fallback when AX unavailable
```

## Performance

| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| axOnlyRatio | 0.5 | 1.0 | +100% |
| avgLatencyMs | 11194 | 404 | -96.4% |

## Stealth Mode

When `PI_COMPUTER_USE_STEALTH=1` or `PI_COMPUTER_USE_STRICT_AX=1` is set, the extension operates without global cursor takeover, making it less intrusive during development.

## Browser-Aware Targeting

The extension detects isolated browser windows and prefers semantic targeting within them, enabling reliable interaction with web-based UIs running in separate browser windows.

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-computer-use` |
| Platform | macOS |
| Latest | v0.1.3 |
| Benchmark | `benchmarks/` directory with regression checks |
