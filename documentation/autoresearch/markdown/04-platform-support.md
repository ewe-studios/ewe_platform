# autoresearch -- Platform Support

## GPU Requirements

### Official Support: Single NVIDIA GPU

The code currently requires a single NVIDIA GPU with CUDA support. It was tested on an **NVIDIA H100**.

**Why single GPU?** The design philosophy is simplicity. No distributed training, no complex configuration files, no multi-node orchestration. One GPU, one file, one metric.

### Flash Attention 3

The training script uses Flash Attention 3 for efficient attention computation:

- **Hopper (H100, compute capability 9.0):** Uses `varunneal/flash-attention-3` kernel from `kernels` package
- **Non-Hopper GPUs:** Falls back to `kernels-community/flash-attn3`

The kernel is loaded dynamically based on the detected compute capability:

```python
cap = torch.cuda.get_device_capability()
repo = "varunneal/flash-attention-3" if cap == (9, 0) else "kernels-community/flash-attn3"
fa3 = get_kernel(repo).flash_attn_interface
```

### Why Not CPU, MPS, AMD Out of the Box?

In principle, the code could support CPU, MPS (Apple Silicon), and AMD ROCm platforms. However, adding these would:

- Bloat the codebase with device detection and fallback logic
- Require platform-specific kernel implementations (Flash Attention alternatives)
- Complicate the single-file design

Karpathy has stated he is not sure he wants to take this on personally. The parent [nanochat](https://github.com/karpathy/nanochat) repository has wider platform support and shows various solutions (Flash Attention 3 fallbacks, generic device support, autodetection, etc.).

## Recommended Forks for Other Platforms

Several community forks have been created for different platforms:

| Fork | Platform | Description |
|------|----------|-------------|
| [miolini/autoresearch-macos](https://github.com/miolini/autoresearch-macos) | MacOS | Apple Silicon / MPS support |
| [trevin-creator/autoresearch-mlx](https://github.com/trevin-creator/autoresearch-mlx) | MacOS | MLX-based implementation |
| [jsegov/autoresearch-win-rtx](https://github.com/jsegov/autoresearch-win-rtx) | Windows | Windows + NVIDIA RTX support |
| [andyluo7/autoresearch](https://github.com/andyluo7/autoresearch) | AMD | AMD ROCm support |

## Running on Smaller Compute

For those trying autoresearch on less powerful hardware (MacBooks, older GPUs, etc.), Karpathy provides the following recommendations:

### 1. Use a Lower-Entropy Dataset

Instead of ClimbMix-400B, use a dataset with narrower scope. The [TinyStories dataset](https://huggingface.co/datasets/karpathy/tinystories-gpt4-clean) contains GPT-4 generated short stories. Because the data has much less entropy, reasonable results can be achieved with much smaller models.

### 2. Decrease Vocabulary Size

Reduce `VOCAB_SIZE` from 8192 down to:

- 4096
- 2048
- 1024
- Or even 256 (byte-level tokenizer after UTF-8 encoding)

This is modified in `prepare.py`.

### 3. Lower Sequence Length

In `prepare.py`, reduce `MAX_SEQ_LEN` significantly — down to 256 on very constrained hardware. As you lower `MAX_SEQ_LEN`, you may want to experiment with increasing `DEVICE_BATCH_SIZE` in `train.py` to compensate. The number of tokens per fwd/bwd pass is the product of these two.

### 4. Reduce Evaluation Data

In `prepare.py`, decrease `EVAL_TOKENS` so that validation loss is evaluated on less data, speeding up the evaluation step.

### 5. Reduce Model Depth

In `train.py`, the primary knob controlling model complexity is `DEPTH` (default: 8). Lower it down to 4 or less. Many other variables are functions of `DEPTH`, so this cascades.

### 6. Use Full Window Attention

Switch `WINDOW_PATTERN` from `"SSSL"` to just `"L"`. The `"SSSL"` pattern uses an alternating banded attention pattern that may be very inefficient on smaller hardware.

### 7. Lower Total Batch Size

Reduce `TOTAL_BATCH_SIZE` significantly, keeping it as a power of 2. Down to `2**14` (~16K) tokens per step or so.

### Summary of Tuning Knobs for Small Hardware

| Parameter | Default | Suggested (Small) | Where |
|-----------|---------|-------------------|-------|
| Dataset | ClimbMix-400B | TinyStories | `prepare.py` (URL) |
| VOCAB_SIZE | 8192 | 256-4096 | `prepare.py` |
| MAX_SEQ_LEN | 2048 | 256-512 | `prepare.py` |
| EVAL_TOKENS | 40 * 524288 | Lower | `prepare.py` |
| DEPTH | 8 | 4 or less | `train.py` |
| WINDOW_PATTERN | "SSSL" | "L" | `train.py` |
| TOTAL_BATCH_SIZE | 2^19 | 2^14 or lower | `train.py` |

## Next

- [05-development](./05-development.md) — setup, running the agent, and notability
- [00-overview](./00-overview.md) — back to overview
