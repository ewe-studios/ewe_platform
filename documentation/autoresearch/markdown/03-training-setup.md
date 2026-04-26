# autoresearch -- Training Setup

## Model Architecture

The model is a decoder-only transformer (GPT), cherry-picked and simplified from [nanochat](https://github.com/karpathy/nanochat). Key architectural choices:

### Default Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `DEPTH` | 8 | Number of transformer layers |
| `ASPECT_RATIO` | 64 | model_dim = depth * ASPECT_RATIO (→ 512) |
| `HEAD_DIM` | 128 | Target head dimension |
| `n_head` | 4 | Derived: model_dim / HEAD_DIM |
| `n_kv_head` | 4 | GQA, same as n_head (no GQA in defaults) |
| `n_embd` | 512 | Derived: rounded up to multiple of HEAD_DIM |
| `WINDOW_PATTERN` | "SSSL" | Sliding window: S=half context, L=full context |
| `MAX_SEQ_LEN` | 2048 | Context length |
| `vocab_size` | 8192 | BPE vocabulary size |

### Architectural Features

- **RMSNorm** — applied to attention inputs, query/key projections, and pre-MLP
- **Rotary Positional Embeddings (RoPE)** — precomputed for 10x the sequence length, base=10000
- **Value Embeddings (ResFormer)** — alternating layers add value embeddings to V projections, gated by an input-dependent linear layer (sigmoid, scaled by 2 → neutral at init)
- **Residual connections with learnable scalars** — each layer uses `lambda_res * x + lambda_x0 * x0` instead of plain residual, allowing the model to interpolate between residual and direct input paths
- **Sliding window attention** — "SSSL" pattern alternates between half-context and full-context attention across layers; the last layer always uses full context
- **Soft-capped logits** — logits are tanh-scaled with softcap=15 before the final softmax
- **Squared ReLU activation** — MLP uses `ReLU(x)^2` (GELU variant)
- **Grouped Query Attention** — supported via `n_kv_head` parameter (defaults to equal `n_head`)

### Model Size

With default settings (DEPTH=8), the model has approximately 50 million parameters. The `ASPECT_RATIO` and `DEPTH` knobs control model complexity — the agent can adjust these.

### Weight Initialization

- Embedding: normal(0, 1)
- LM head: normal(0, 0.001)
- Attention/MLP matrices: uniform(-s, s) where s = sqrt(3) / sqrt(n_embd)
- Attention output projections: zeros (residual path carries signal initially)
- MLP output projections: zeros (same rationale)
- Per-layer scalars: resid_lambdas=1.0, x0_lambdas=0.1
- Value embedding gates: zeros (sigmoid(0)=0.5, scaled by 2 → neutral)
- Embeddings cast to bfloat16

## Optimizer: Muon + AdamW

The optimizer uses a hybrid approach:

| Parameter Type | Optimizer | LR | Notes |
|---------------|-----------|----|-------|
| LM head (unembedding) | AdamW | 0.004 | beta=(0.8, 0.95), eps=1e-10, no weight decay |
| Token embedding | AdamW | 0.6 | beta=(0.8, 0.95), eps=1e-10, no weight decay |
| Value embeddings | AdamW | 0.6 | beta=(0.8, 0.95), eps=1e-10, no weight decay |
| Per-layer scalars (resid) | AdamW | 0.005 | beta=(0.8, 0.95), eps=1e-10, no weight decay |
| Per-layer scalars (x0) | AdamW | 0.5 | beta=(0.96, 0.95), eps=1e-10, no weight decay |
| Matrix params (2D, by shape) | Muon | 0.04 | momentum=0.95, ns_steps=5, beta2=0.95, cautious weight decay=0.2 |

### Muon Details

- **Polar Express** — Newton-Schulz iteration for matrix orthogonalization using precomputed polynomial coefficients (5 iterations default)
- **NorMuon** — variance reduction via per-layer normalization using tracked second moment
- **Cautious weight decay** — weight decay is only applied when the update direction agrees with the parameter sign (mask-based)
- **Fused kernels** — both AdamW and Muon steps use `torch.compile` fused kernels for performance
- **Shape-based grouping** — matrix parameters are grouped by shape, and Muon steps stack same-shaped parameters together for batched orthogonalization

### Learning Rate Scheduling

- **Warmup** — fraction of time budget (default: 0, no warmup)
- **Warmdown** — fraction of time budget for cooldown (default: 0.5, i.e., last 50% of training)
- **Final LR fraction** — final LR as fraction of initial (default: 0, goes to zero)
- **Muon momentum** — ramps from 0.85 to 0.95 over first 300 steps
- **Weight decay** — linearly decays to zero over the training run
- **Dimension scaling** — AdamW LRs are scaled by 1/sqrt(model_dim/768) to account for different model widths

## Training Loop

### Batch Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `TOTAL_BATCH_SIZE` | 2^19 (~524K) | Tokens per optimizer step |
| `DEVICE_BATCH_SIZE` | 128 | Per-device batch size |
| `MAX_SEQ_LEN` | 2048 | Sequence length |
| Tokens per fwd/bwd pass | 262,144 | DEVICE_BATCH_SIZE * MAX_SEQ_LEN |
| Gradient accumulation steps | 2 | TOTAL_BATCH_SIZE / tokens_per_pass |

### Dataloader

- **BOS-aligned** — every row starts with the BOS token
- **Best-fit packing** — documents are packed using best-fit bin packing to minimize cropping
- **100% utilization** — no padding; when no document fits remaining space, the shortest document is cropped to fill exactly
- **Infinite iterator** — yields batches forever, tracking epoch number
- **Pinned memory** — CPU-to-GPU transfer uses pinned memory with non-blocking copy

### Time Budget Enforcement

- Training runs for exactly **300 seconds** (5 minutes) of wall-clock time
- First 10 steps are excluded from timing (to avoid counting CUDA compilation)
- The loop checks `total_training_time >= TIME_BUDGET` after step 10
- Fast-fail: if loss is NaN or exceeds 100, the process exits immediately

### GC Management

- Python's garbage collector causes ~500ms stalls
- GC is disabled after step 0 (`gc.freeze()` + `gc.disable()`)
- Manual `gc.collect()` every 5000 steps

### Output Format

When training finishes, a summary is printed:

```
val_bpb:          0.997900
training_seconds: 300.1
total_seconds:    325.9
peak_vram_mb:     45060.2
mfu_percent:      39.80
total_tokens_M:   499.6
num_steps:        953
num_params_M:     50.3
depth:            8
```

The agent extracts `val_bpb` and `peak_vram_mb` from this output using `grep`.

## Next

- [04-platform-support](./04-platform-support.md) — GPU requirements and platform forks
- [02-agent-research](./02-agent-research.md) — how the autonomous agent works
