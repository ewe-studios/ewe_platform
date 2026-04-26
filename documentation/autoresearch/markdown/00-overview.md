# autoresearch -- Overview

## What autoresearch Is

autoresearch is an autonomous AI research system created by Andrey Karpathy (@karpathy) in March 2026. The core idea is simple but radical: give an AI agent a small but real LLM training setup and let it experiment autonomously overnight.

The agent modifies training code, runs a 5-minute training job, checks whether the result improved, keeps or discards the changes, and repeats. You wake up in the morning to a log of experiments and (hopefully) a better model.

As Karpathy puts it:

> *One day, frontier AI research used to be done by meat computers in between eating, sleeping, having other fun, and synchronizing once in a while using sound wave interconnect in the ritual of "group meeting". That era is long gone. Research is now entirely the domain of autonomous swarms of AI agents running across compute cluster megastructures in the skies.*

## The Philosophy

### You Don't Write Research Code — You Write Research Instructions

In a traditional ML research workflow, the researcher writes and modifies the training code directly. In autoresearch, the human writes `program.md` — a Markdown file that acts as instructions for an AI agent. The agent is the one that reads, understands, and modifies `train.py`.

This flips the workflow:

- **Traditional researcher:** reads paper → writes code → runs experiment → analyzes results → repeats
- **autoresearch human:** writes `program.md` → spins up agent → sleeps → wakes up to results

The agent is fully autonomous. Once the loop starts, it does not pause to ask for permission. It runs until the human interrupts it.

### Fixed Time Budget as an Experimental Design Choice

Every training run is exactly 5 minutes of wall-clock time (excluding startup and compilation). This is intentional:

1. **All experiments are directly comparable.** A model with different architecture, batch size, or optimizer is measured under identical conditions.
2. **The agent finds the best model for your platform within that budget.** It is not optimizing for asymptotic performance — it is optimizing for what works best in 5 minutes on your hardware.
3. **Predictable throughput.** Approximately 12 experiments per hour, roughly 100 experiments per night.

The trade-off is that results are not comparable across different compute platforms. An H100 and a MacBook will produce different absolute numbers.

### Simplicity and Scope

The entire project is deliberately kept small — just three files that matter. The agent only modifies `train.py`. This keeps the scope manageable and diffs reviewable. There is no distributed training, no complex configuration files, no external orchestration. One GPU, one file, one metric.

## The Vision

autoresearch is not just a training script — it is a prototype of a different way to do research. The `program.md` file is described as a "super lightweight skill." Over time, one could iterate on it to discover the "research org code" that achieves the fastest research progress: adding more agents, refining the experimentation strategy, and building up a body of knowledge about how to autonomously improve models.

The repo is described as "the story of how it all began" — the starting point for a future where AI research is conducted by autonomous agent swarms rather than individual humans.

## Key Properties

| Property | Detail |
|----------|--------|
| Author | Andrey Karpathy (@karpathy) |
| Date | March 2026 |
| License | MIT |
| Compute | Single NVIDIA GPU (tested on H100) |
| Language | Python 3.10+ |
| Package manager | uv |
| Training time per experiment | Fixed 5 minutes |
| Metric | val_bpb (validation bits per byte), lower is better |
| Experiments per night | ~100 |
| Codebase origin | Simplified single-GPU version of [nanochat](https://github.com/karpathy/nanochat) |

## Next

- [01-architecture](./01-architecture.md) — project structure and the three key files
- [02-agent-research](./02-agent-research.md) — how the autonomous agent works
- [03-training-setup](./03-training-setup.md) — model, optimizer, and training details
