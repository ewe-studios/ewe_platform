---
title: "Pi Extensions -- pi-review-loop"
---

# pi-review-loop

**Automated code review loop -- review until no issues remain.**

pi-review-loop repeatedly prompts the agent to review its own work until it confirms no issues remain. Each iteration the agent finds bugs, fixes them, and reviews again.

## Usage

```
> /review-start

Review mode (1/7)  ← status appears in footer

[agent reviews, finds bug, fixes it]

Review mode (2/7)

[agent reviews again]

"No issues found."

✓ Review complete -- no issues after 3 iterations
```

## How It Works

1. The extension enters review mode with a maximum iteration count
2. Each iteration: the agent reviews all recent changes, identifies issues, and fixes them
3. If the agent finds no issues, the loop terminates early
4. If the maximum iterations are reached with issues still found, the loop stops anyway

## Why It Exists

Self-review is one of the most effective quality practices in software engineering. pi-review-loop automates this practice, ensuring the agent reviews its work with the same rigor a human reviewer would -- but without waiting for a PR.

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-review-loop` |
| Trigger | `/review-start` |
| Pattern | Automated self-review loop |
| Max iterations | Configurable (default: 7) |
