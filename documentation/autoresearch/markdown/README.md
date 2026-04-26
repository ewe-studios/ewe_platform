# autoresearch -- Documentation Index

**autoresearch** is an autonomous AI research system by Andrey Karpathy. An AI agent experiments with LLM training code overnight, modifying `train.py`, training for 5 minutes, checking if results improved, and iterating.

## Documents

### Foundation

| Document | What It Covers |
|----------|---------------|
| [00-overview.md](./00-overview.md) | What autoresearch is, philosophy, the vision from @karpathy |
| [01-architecture.md](./01-architecture.md) | Project structure, the 3 key files, how they interact |

### Agent Research

| Document | What It Covers |
|----------|---------------|
| [02-agent-research.md](./02-agent-research.md) | How the agent works, autonomous experimentation, val_bpb metric |
| [03-training-setup.md](./03-training-setup.md) | Model architecture, optimizer, fixed 5-min time budget, single-GPU |

### Platform & Development

| Document | What It Covers |
|----------|---------------|
| [04-platform-support.md](./04-platform-support.md) | GPU requirements, smaller compute forks, recommendations |
| [05-development.md](./05-development.md) | Setup, running the agent, notability |

## Quick Orientation

```
prepare.py       ← Constants, data prep, tokenizer, dataloader, eval (READ-ONLY)
train.py         ← GPT model, optimizer (Muon+AdamW), training loop (AGENT EDITS THIS)
program.md       ← Lightweight "skill" file — agent instructions (HUMAN EDITS THIS)
```

The agent loop: modify `train.py` → train for 5 minutes → check `val_bpb` → keep or discard → repeat. Approximately 12 experiments per hour, ~100 per night.

## Source

`/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AIResearch/autoresearch/`
