# 14 - MMAP: How llama.cpp Achieves Instant Model Loading

## Overview

One of llama.cpp's most powerful features is its ability to "load" multi-gigabyte models in milliseconds using memory-mapped I/O (mmap). This document explains the complete mmap pipeline — from the GGUF file format's memory-mappable design, through the OS-level mmap implementation, to the zero-copy tensor access pattern that makes it all work.

---

## 1. The Problem: Traditional Model Loading is Slow

A typical 70B parameter model quantized to Q4_K occupies ~40GB on disk. Traditional loading:

```
Read file into heap memory:    40GB disk I/O → 40GB RAM allocation → seconds to minutes
Parse metadata:                Deserialize headers, tensor info
Copy to compute buffers:       40GB RAM → GPU VRAM (another copy)
```

Total: Multiple minutes, 80GB+ peak memory usage (disk buffer + compute buffer).

### MMAP Solution

```
mmap() syscall:               ~1ms (maps virtual address range, no data transfer)
Parse GGUF header:            ~1ms (small fixed-size header)
Set tensor pointers:          ~0ms (direct pointers into mmap'd region)
Lazy page-in on access:       OS pages in data on demand, only what's actually read
```

Total: ~2ms "load" time, memory usage = only pages actively being accessed.

---

## 2. GGUF: A Memory-Mappable File Format

The GGUF format is specifically designed for mmap:

```
┌─────────────────────────┐
│ GGUF Header (32 bytes)  │  Magic number, version, tensor count, metadata KV count
├─────────────────────────┤
│ Metadata KV Pairs       │  Model architecture, tokenizer, chat template, etc.
│ (variable size)         │  
├─────────────────────────┤
│ Tensor Info Array        │  Per-tensor: name, dimensions, type, FILE OFFSET
│ (variable size)         │  
├─────────────────────────┤
│ Alignment Padding        │  Padded to alignment boundary (typically 32 bytes)
├─────────────────────────┤
│ Tensor Data              │  Raw quantized weights, contiguous, aligned
│ (bulk of file)          │  Tensors stored in the order listed in info array
│                          │  Each tensor aligned to type-specific boundary
└─────────────────────────┘
```

Key design decisions for mmap:
1. **Tensor data is contiguous and aligned** — mmap'd pages map directly to tensor storage
2. **Tensor info stores file offsets** — pointer arithmetic from mmap base address
3. **No deserialization needed** — quantized data is stored in compute-ready format
4. **Alignment padding** — ensures tensor data falls on page boundaries

---

## 3. The MMAP Implementation

### Source: `src/llama-mmap.cpp` (432 lines)

### Platform Abstraction

```cpp
struct llama_mmap {
    void * addr;       // Base address of mapped region
    size_t size;       // Size of mapping
    
    // Platform-specific implementation
    struct impl;
    std::unique_ptr<impl> pimpl;
};
```

### POSIX Implementation (Linux/macOS)

```cpp
// Core mmap call
addr = mmap(NULL, file->size(), PROT_READ, flags, fd, 0);

// Flags:
// MAP_SHARED    — Read-only shared mapping (no copy-on-write overhead)
// MAP_POPULATE  — Eagerly page in (when prefetch requested)
```

### Prefetch Optimization

```cpp
// Advise kernel about access patterns for optimal paging
if (prefetch > 0) {
    // "We will need this data soon" — triggers background page-in
    posix_madvise(addr, prefetch_size, POSIX_MADV_WILLNEED);
}

// On NUMA systems: prevent sequential readahead across NUMA boundaries
if (is_numa) {
    posix_madvise(addr, file_size, POSIX_MADV_RANDOM);
}
```

When `prefetch = -1` (entire file), `MAP_POPULATE` is used on Linux to eagerly fault in all pages during the mmap call. This trades "instant" mapping for predictable latency (all I/O happens upfront).

### Memory Locking (Optional)

```cpp
// llama_mlock — prevents OS from paging out model data
struct llama_mlock {
    struct impl;
    std::unique_ptr<impl> pimpl;
    
    void init(void * addr);
    void grow_to(size_t target_size);  // Incremental locking
};

// POSIX: mlock(addr, size) — locks pages in physical RAM
// Windows: VirtualLock(addr, size) — requires working set adjustment
```

`use_mlock = true` prevents the OS from evicting model pages under memory pressure. Critical for production deployments where latency spikes from page faults are unacceptable.

---

## 4. Model Loading Pipeline

### Step 1: GGUF Metadata Initialization

**Source**: `src/llama-model-loader.cpp` (lines 508-650)

```cpp
// Load GGUF metadata WITHOUT allocating tensor data
gguf_init_params params = { .no_alloc = true, .ctx = &ctx };
metadata_ptr.reset(gguf_init_from_file(fname.c_str(), params));

// Build index: tensor name → (file_index, byte_offset, tensor_ptr)
for (ggml_tensor * cur = ggml_get_first_tensor(ctx); cur; cur = ggml_get_next_tensor(ctx, cur)) {
    // Calculate byte offset: GGUF data section start + tensor-specific offset
    size_t offs = gguf_data_offset + tensor_offset;
    weights_map.emplace(tensor_name, llama_tensor_weight(file, idx, metadata, cur));
}
```

The critical insight: `no_alloc = true` means **no tensor data is read**. Only the GGUF header and metadata are parsed. This is the ~1ms operation.

### Step 2: MMAP Initialization

**Source**: `src/llama-model-loader.cpp` (lines 1323-1354)

```cpp
void llama_model_loader::init_mappings(bool prefetch, llama_mlocks * mlock_mmaps) {
    if (use_mmap) {
        for (const auto & file : files) {
            // Create mmap for entire file
            auto mapping = std::make_unique<llama_mmap>(
                file.get(),
                prefetch ? -1 : 0,  // -1 = prefetch entire file, 0 = lazy
                is_numa
            );
            mappings.emplace_back(std::move(mapping));
            
            // Optional: lock mapping in physical RAM
            if (mlock_mmaps) {
                auto mlock_mmap = std::make_unique<llama_mlock>();
                mlock_mmap->init(mapping->addr());
                mlock_mmaps->emplace_back(std::move(mlock_mmap));
            }
        }
    }
}
```

### Step 3: Zero-Copy Tensor Pointer Assignment

**Source**: `src/llama-model-loader.cpp`

```cpp
void llama_model_loader::load_data_for(struct ggml_tensor * cur) {
    const auto & w = require_weight(ggml_get_name(cur));
    
    if (use_mmap) {
        // ZERO COPY: Point tensor directly into mmap'd region
        const auto & mapping = mappings.at(w.idx);
        cur->data = (uint8_t *)mapping->addr() + w.offs;
        // That's it! No memcpy, no allocation, no deserialization
    } else {
        // Fallback: traditional file read
        file->seek(w.offs, SEEK_SET);
        file->read_raw(cur->data, ggml_nbytes(cur));
    }
}
```

This is where the magic happens. Each tensor's `data` pointer is set to point directly into the mmap'd file. **There is no copy.** The tensor "data" is the file itself, mapped into the process's virtual address space.

### Step 4: Backend Buffer Integration (GPU Offloading)

**Source**: `src/llama-model.cpp` (lines 7801-7820)

```cpp
if (ml.use_mmap && use_mmap_buffer && buffer_from_host_ptr_supported) {
    // Get the byte range in the mmap that contains this layer's tensors
    void * addr = nullptr;
    size_t first, last;
    ml.get_mapping_range(&first, &last, &addr, idx, ctx);
    
    // Create a GPU buffer as a VIEW into the mmap'd region
    ggml_backend_buffer_t buf = ggml_backend_dev_buffer_from_host_ptr(
        dev,                           // GPU device
        (char *) addr + first,         // Pointer into mmap region
        last - first,                  // Size of tensor data
        max_size                       // Alignment
    );
    buf_map.emplace(idx, buf);
}
```

On supported platforms (CUDA with managed memory, Metal with shared memory), the GPU can **read directly from the mmap'd region** without an explicit CPU→GPU copy. The GPU's MMU handles the DMA transfer from disk when the GPU accesses the data.

---

## 5. Memory Access Patterns

### Lazy Paging (Default)

```
Time 0ms:  mmap() returns immediately
           Virtual address range [0, 40GB) mapped but NOT resident
           
Time 1ms:  Application accesses layer 0 weights
           → Page fault → OS reads 4KB page from disk → Page now resident
           
Time 2ms:  Application accesses layer 1 weights  
           → Page fault → OS reads pages for layer 1
           
...        Only accessed layers are paged in
           Unused layers (e.g., skipped by GPU offloading) never touch disk
```

### Eager Paging (with MAP_POPULATE or MADV_WILLNEED)

```
Time 0ms:  mmap(MAP_POPULATE) — kernel begins paging in entire file
           Background DMA transfer starts
           
Time 0-5s: Pages fault in as background I/O completes
           Application can start using early layers while late layers still loading
           
Time 5s:   All pages resident — subsequent access is zero-copy RAM speed
```

### Interaction with Quantization

Quantized models are especially mmap-friendly:

| Quantization | Model Size (7B) | Memory per Page (4KB) |
|-------------|-----------------|----------------------|
| F32 | ~28 GB | 1 page = 1024 floats |
| F16 | ~14 GB | 1 page = 2048 halfs |
| Q8_0 | ~7 GB | 1 page = 4096 quants |
| Q4_K | ~4 GB | 1 page = ~8192 quants |
| Q2_K | ~2.7 GB | 1 page = ~12288 quants |

Smaller quantization = fewer pages to fault in = faster effective loading.

---

## 6. Split Model Support

Large models can be split across multiple files:

```cpp
// Load from multiple GGUF shards
llama_model * model = llama_model_load_from_splits(
    paths, n_paths, model_params
);

// Each shard gets its own mmap
for (const auto & file : files) {
    mappings.emplace_back(std::make_unique<llama_mmap>(file.get(), ...));
}

// Tensor weights reference their source file index
struct llama_tensor_weight {
    int idx;           // Which file (mmap) this tensor lives in
    size_t offs;       // Byte offset within that file
    ggml_tensor * tensor;
};
```

---

## 7. Model Parameters Controlling MMAP

```cpp
struct llama_model_params {
    bool use_mmap;        // Enable memory-mapped I/O (default: true)
    bool use_direct_io;   // Use O_DIRECT instead (Linux, bypasses page cache)
    bool use_mlock;       // Lock mmap'd pages in RAM (prevent swapping)
    // ...
};
```

### `use_mmap = true` (Default)
- Fastest "load" time
- OS manages page cache automatically
- Pages can be evicted under memory pressure (unless mlocked)
- Multiple processes can share the same physical pages (MAP_SHARED)

### `use_mmap = false`
- Traditional file read into allocated buffers
- Predictable memory usage (no page cache interaction)
- Useful when mmap is unavailable (some network filesystems)

### `use_direct_io = true` (Linux only)
- Bypasses the OS page cache entirely (O_DIRECT flag)
- Reads go directly from disk to application buffers
- Useful when: model is too large for page cache, or other processes need RAM
- Takes precedence over `use_mmap` when supported

### `use_mlock = true`
- After mmap, lock all pages in physical RAM
- Prevents OS from evicting pages under memory pressure
- Guarantees no page faults during inference (deterministic latency)
- Requires sufficient RAM + OS privileges (may need `ulimit -l unlimited`)

---

## 8. Shared Memory Between Processes

Because mmap uses `MAP_SHARED`, multiple processes loading the same model file share the same physical memory pages:

```
Process A:  mmap("model.gguf") → virtual addr 0x1000000
Process B:  mmap("model.gguf") → virtual addr 0x5000000
                                    ↓
                            Same physical pages!
```

This means running 4 inference servers on the same machine with the same model uses ~1x model memory, not 4x. The OS page cache handles deduplication transparently.

---

## 9. Rust Bindings Integration

### Source: `infrastructure/llama-cpp/src/model/params.rs`

The Rust bindings directly expose mmap controls:

```rust
impl LlamaModelParams {
    pub fn use_mmap(&self) -> bool {
        self.params.use_mmap
    }
    
    pub fn with_use_mmap(mut self, use_mmap: bool) -> Self {
        self.params.use_mmap = use_mmap;
        self
    }
    
    pub fn with_use_mlock(mut self, use_mlock: bool) -> Self {
        self.params.use_mlock = use_mlock;
        self
    }
}
```

### Foundation_ai Configuration

```rust
// In LlamaBackendConfig (builder pattern):
let config = LlamaBackendConfig::builder()
    .use_mmap(true)       // Default: true
    .use_mlock(false)     // Default: false (requires privileges)
    .n_gpu_layers(99)     // Offload all layers to GPU
    .build();
```

---

## 10. Performance Characteristics

### Load Time Comparison (7B Q4_K model, ~4GB, NVMe SSD)

| Method | Time | Peak Memory |
|--------|------|-------------|
| Traditional read | ~2-4s | ~8GB (file buffer + tensors) |
| mmap (lazy) | ~2ms | ~0MB initially, grows on access |
| mmap (prefetch) | ~2ms + ~1-2s background | ~4GB (as pages fault in) |
| mmap + mlock | ~2ms + ~1-2s mlock | ~4GB (locked) |

### Inference Latency Impact

| Scenario | First Token Latency | Subsequent Tokens |
|----------|--------------------|--------------------|
| mmap, cold cache | Higher (page faults on first access) | Normal (pages now resident) |
| mmap, warm cache | Normal | Normal |
| mmap + mlock | Normal (all pages locked) | Normal |
| mmap + prefetch | Normal (pages pre-faulted) | Normal |

### When NOT to Use MMAP

1. **Network filesystems** — mmap over NFS/CIFS can be unreliable or slow
2. **Memory-constrained systems** — page cache competes with application memory
3. **Spinning disks with random access** — mmap random faults are slow on HDD
4. **Containers with memory limits** — page cache may count against cgroup memory limit

---

## 11. Key Takeaways for foundation_ai

1. **Always default to `use_mmap = true`** — it's faster and more memory-efficient
2. **Expose `use_mlock` as an option** for production deployments that need deterministic latency
3. **Document the "lazy loading" behavior** — users may be surprised that the model "loads instantly" but the first inference is slower (page faults)
4. **Shared memory benefit** — mention in docs that multiple model instances sharing the same file share physical memory
5. **Split model support** — ensure `LlamaModels` handles multi-file GGUF models correctly
6. **GPU offloading + mmap** — when GPU supports `buffer_from_host_ptr`, tensors go directly from mmap to GPU without intermediate copies

---

_Created: 2026-04-07_
_Source: llama.cpp src/llama-mmap.cpp, src/llama-model-loader.cpp, src/llama-model.cpp, include/llama.h_
