# Progress - 03 Candle Integration

_Last updated: 2026-04-12_

**Status:** ⬜ Pending — 0 / 18 tasks (0%)

Alternative `ModelProvider` using HuggingFace Candle for native Rust
inference with safetensors (CUDA / Metal).

## Blocked On

- **01 llamacpp-integration** — reuses the same `ModelProvider` abstraction
  and type surface; needs 01 to finalise those contracts first

## Next Action

Wait for 01. Then start at `start.md`.

See [`feature.md`](./feature.md) for the 18-task breakdown and the
candle/safetensors design notes.
