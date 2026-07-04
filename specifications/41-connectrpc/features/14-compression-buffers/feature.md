---
feature: "Compression system + buffer pools (D06)"
description: "Compressor trait, gzip/zstd/brotli, negotiation, size limits, per-worker BufferPool with freeze/PooledFrame"
status: "complete"
priority: "high"
phase: 1
depends_on: ["12-error-model"]
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 14-compression-buffers: Compression system + buffer pools (D06)

## Description

Pluggable per-message compression plus the buffer-pool ownership model: worker = executor thread, write-path scratch from the thread-local pool, pipe-bound frames recycled via freeze/PooledFrame with the lock-free origin return lane.

## Normative sources (single source of truth — read before writing code)

- decisions/06-compression.md — entire doc is normative (negotiate_compression is THE single definition; §Buffer Pool ownership model)

## Scope

- Compressor trait + GzipCompressor (default) + feature-gated Zstd/Brotli; RS10 streaming reset-able forms
- CompressionRegistry (this one stays — orthogonal to codecs) + negotiate_compression + NegotiatedCompression
- SizeLimits (read/send/compress_min — connect-go-parity unlimited defaults)
- BufferPool: thread-local per worker, LIFO + bounded return lane; freeze(buf) -> Bytes via PooledFrame owner (origin-lane recycling); with_worker_buffer accessor
- Invariants enforced in review: pooled buffers never cross a yield as borrows; pipes carry owned Bytes

## Acceptance criteria

- Per-message compression honors compress_min_bytes; send_max checked after compression (H10)
- A frozen frame's final drop on another thread returns the allocation to the origin pool (test with counters)
- Compression bomb bounded by read_max_bytes when configured
