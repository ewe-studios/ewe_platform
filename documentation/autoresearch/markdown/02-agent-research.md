# autoresearch -- Agent Research Process

## How the Agent Works

The AI agent (Claude, Codex, or any capable coding agent) operates as a fully autonomous researcher. Once set up, it enters an endless loop of experimentation that runs until the human manually stops it.

The agent works on a dedicated git branch (e.g., `autoresearch/mar5`), committing each experiment and keeping only the commits that improve results.

## The Setup Phase

Before experimentation begins, the agent works with the human to:

1. **Agree on a run tag** — based on today's date (e.g., `mar5`). The branch `autoresearch/<tag>` must not already exist.
2. **Create the branch** — `git checkout -b autoresearch/<tag>` from master.
3. **Read in-scope files** — `README.md`, `prepare.py`, and `train.py` for full context.
4. **Verify data exists** — Check that `~/.cache/autoresearch/` contains data shards and a tokenizer.
5. **Initialize `results.tsv`** — Create the TSV file with a header row.
6. **Run the baseline** — The first experiment always runs `train.py` as-is to establish a baseline.

## The Experiment Loop

```
LOOP FOREVER:
  1. Look at git state (current branch/commit)
  2. Tune train.py with an experimental idea
  3. git commit
  4. Run: uv run train.py > run.log 2>&1
  5. Read results: grep "^val_bpb:\|^peak_vram_mb:" run.log
  6. If grep empty → run crashed → check stack trace → attempt fix or discard
  7. Record results in results.tsv (tab-separated, NOT committed)
  8. If val_bpb improved (lower) → keep commit, advance branch
  9. If val_bpb same or worse → git reset to start point, discard
```

Each experiment takes approximately 5 minutes (plus a few seconds for startup and eval overhead). If a run exceeds 10 minutes, it is killed and treated as a failure.

## The Results Log

Results are recorded in `results.tsv`, a tab-separated file with 5 columns:

| Column | Format | Example |
|--------|--------|---------|
| `commit` | Short git hash (7 chars) | `a1b2c3d` |
| `val_bpb` | Float, 6 decimal places | `0.997900` |
| `memory_gb` | Float, 1 decimal place | `44.0` |
| `status` | `keep`, `discard`, or `crash` | `keep` |
| `description` | Short text | `increase LR to 0.04` |

The TSV file is intentionally left untracked by git — it is a local experiment log, not part of the codebase.

## The val_bpb Metric

**val_bpb** (validation bits per byte) is the sole evaluation metric. It measures how many bits are needed, on average, to encode each byte of validation data using the trained model.

### Why val_bpb instead of perplexity?

- **Vocabulary-size independent.** Perplexity depends on vocabulary size — changing the tokenizer makes perplexity numbers incomparable. val_bpb normalizes to bytes, so architectural changes (different vocab sizes, different tokenizers) can be fairly compared.
- **Computationally simple.** Sum per-token cross-entropy in nats, sum target byte lengths, convert nats/byte to bits/byte.
- **Lower is better.** A lower val_bpb means the model compresses the data more efficiently.

### How it is computed

The `evaluate_bpb()` function in `prepare.py`:

1. Loads the token bytes lookup table (mapping each token ID to its UTF-8 byte length)
2. Runs the model over the pinned validation shard (shard_06542)
3. For each token: computes cross-entropy loss, looks up byte length, masks out special tokens (byte length = 0)
4. Sums total nats and total bytes, converts: `nats / (ln(2) * bytes) = bits per byte`
5. Uses fixed `MAX_SEQ_LEN` (2048) so results are comparable across configurations

### The evaluation harness is fixed

The agent **cannot modify** `evaluate_bpb()` in `prepare.py`. This function is the ground truth — it ensures all experiments are measured against the same standard.

## Agent Rules

### What the agent CAN do:

- Modify `train.py` — everything is fair game: model architecture, optimizer, hyperparameters, training loop, batch size, model size, activation functions, attention patterns, etc.

### What the agent CANNOT do:

- Modify `prepare.py` — it is read-only
- Install new packages or add dependencies — only what is in `pyproject.toml`
- Modify the evaluation harness — `evaluate_bpb` is the ground truth metric

## Behavioral Directives

The agent is given strict behavioral rules in `program.md`:

- **NEVER STOP.** Once the loop begins, the agent does not pause to ask the human if it should continue. It runs indefinitely until manually interrupted.
- **Autonomous judgment.** If a run crashes, the agent uses its own judgment: if it is something easy to fix (a typo, a missing import), fix it and re-run. If the idea itself is broken, skip it and move on.
- **Simplicity criterion.** All else being equal, simpler is better. A 0.001 val_bpb improvement that adds 20 lines of hacky code is probably not worth it. Deleting code and getting equal or better results is a win.
- **VRAM is a soft constraint.** Some increase is acceptable for meaningful val_bpb gains, but it should not blow up dramatically.
- **Rarely rewind.** The agent advances the branch on improvements and resets on failures. Rewinding beyond the immediate experiment is discouraged.

## Throughput

| Metric | Value |
|--------|-------|
| Time per experiment | ~5 minutes (+ startup overhead) |
| Experiments per hour | ~12 |
| Experiments per night (8 hours) | ~100 |
| Timeout threshold | 10 minutes (kill and treat as failure) |

## Next

- [03-training-setup](./03-training-setup.md) — model architecture and optimizer
- [00-overview](./00-overview.md) — back to overview
