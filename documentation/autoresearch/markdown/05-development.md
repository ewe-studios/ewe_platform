# autoresearch -- Development Guide

## Setup

### Prerequisites

- **Python 3.10+**
- **uv** package manager
- **Single NVIDIA GPU** (tested on H100)
- **Git** (the agent uses branches and commits extensively)

### Installation

```bash
# 1. Install uv (if you don't already have it)
curl -LsSf https://astral.sh/uv/install.sh | sh

# 2. Clone and enter the repo
git clone https://github.com/karpathy/autoresearch.git
cd autoresearch

# 3. Install dependencies
uv sync

# 4. Download data and train tokenizer (one-time, ~2 min)
uv run prepare.py

# 5. Verify setup with a manual training run (~5 min)
uv run train.py
```

### Dependencies

| Package | Purpose |
|---------|---------|
| `torch==2.9.1` | PyTorch (CUDA 12.8) |
| `kernels>=0.11.7` | Flash Attention 3 kernel loading |
| `rustbpe>=0.1.0` | BPE tokenizer training |
| `tiktoken>=0.11.0` | Token encoding/decoding |
| `numpy>=2.2.6` | Array operations |
| `pyarrow>=21.0.0` | Parquet file reading |
| `requests>=2.32.0` | Data download from HuggingFace |
| `matplotlib>=3.10.8` | Plotting (for progress visualization) |
| `pandas>=2.3.3` | Data manipulation |

All dependencies are managed by `uv` via `pyproject.toml`. The project does not use pip or conda.

### Data Storage

All data and tokenizer files are stored in `~/.cache/autoresearch/`:

```
~/.cache/autoresearch/
├── data/
│   ├── shard_00000.parquet
│   ├── shard_00001.parquet
│   ├── ...
│   └── shard_06542.parquet   # Pinned validation shard
└── tokenizer/
    ├── tokenizer.pkl          # tiktoken pickle
    └── token_bytes.pt         # Token byte lengths (PyTorch tensor)
```

By default, 10 training shards are downloaded. To download more (or all), use:

```bash
uv run prepare.py --num-shards 100   # 100 training shards
uv run prepare.py --num-shards -1    # All shards (6543 total)
```

## Running the Agent

### Manual Mode (Single Experiment)

```bash
uv run train.py
```

This runs a single 5-minute training experiment with the current `train.py` code. Results are printed at the end.

### Autonomous Research Mode

1. **Spin up an AI agent** — Claude, Codex, or any capable coding agent. Point it at the autoresearch repository and disable all permissions (the agent needs full file and shell access).

2. **Initial prompt:**
   ```
   Hi, have a look at program.md and let's kick off a new experiment!
   Let's do the setup first.
   ```

3. **Let it run** — The agent will:
   - Set up a new branch (`autoresearch/<tag>`)
   - Run the baseline experiment
   - Enter the autonomous experimentation loop
   - Continue until you manually stop it

The `program.md` file acts as the agent's "skill" — it contains all the instructions needed to run experiments autonomously.

### Monitoring

While the agent runs, you can monitor progress:

```bash
# Check latest results
cat results.tsv

# View experiment output
tail -f run.log

# Watch val_bpb trend
grep "^val_bpb:" run.log
```

### Stopping the Agent

Since the agent is instructed to never stop on its own, you must manually interrupt it (e.g., `Ctrl+C` or terminate the agent process).

## Notability

### The Notability Concept

The fixed 5-minute time budget makes experiments non-comparable across different compute platforms. An H100 will achieve different absolute val_bpb numbers than a MacBook or an older GPU. This means:

- **Results are platform-specific.** The agent finds the best model for *your* hardware within the time budget.
- **Cross-platform comparison is not meaningful.** A val_bpb of 0.99 on an H100 is not directly comparable to 0.99 on an RTX 4090.
- **This is by design.** The time budget optimizes for practical deployment on a specific platform rather than asymptotic research results.

### Reproducibility

- **Random seed:** Fixed at 42 (`torch.manual_seed(42)` and `torch.cuda.manual_seed(42)`)
- **Data order:** Deterministic through the dataloader's best-fit packing
- **Results should be reproducible** on the same hardware with the same code

### The Agent as Co-Researcher

The real value of autoresearch is not in any single experiment, but in the accumulation of knowledge across many experiments. The `results.tsv` file becomes a record of what works and what does not. The `program.md` file becomes a refined research strategy over time.

## Project Origins

- **Source repository:** [karpathy/autoresearch](https://github.com/karpathy/autoresearch)
- **Author:** Andrey Karpathy (@karpathy)
- **Date:** March 2026
- **License:** MIT
- **Based on:** Simplified single-GPU version of [nanochat](https://github.com/karpathy/nanochat)
- **Training data:** [ClimbMix-400B](https://huggingface.co/datasets/karpathy/climbmix-400b-shuffle)

## Related Resources

- [Tweet: Project announcement](https://x.com/karpathy/status/2029701092347630069)
- [Tweet: Follow-up context](https://x.com/karpathy/status/2031135152349524125)
- [Dummy's Guide to autoresearch](https://x.com/hooeem/status/2030720614752039185)
- [nanochat (parent project)](https://github.com/karpathy/nanochat)

## Next

- [00-overview](./00-overview.md) — back to overview
- [02-agent-research](./02-agent-research.md) — how the autonomous agent works
