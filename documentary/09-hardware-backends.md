# 09 - Hardware Backends

This document covers the hardware acceleration backends available in the llama.cpp integration, how they are configured at build time and runtime, and the platform-specific considerations for each.

## Backend Architecture

llama.cpp uses GGML's backend system to abstract hardware acceleration. Each backend registers buffer types and compute capabilities. The build system (`infrastructure_llama_bindings/build.rs`) selects which backends to compile, and the runtime discovers available devices.

```
+------------------+     +------------------+     +------------------+
|   CPU Backend    |     |   CUDA Backend   |     |  Vulkan Backend  |
|  (always built)  |     | (feature: cuda)  |     | (feature: vulkan)|
+--------+---------+     +--------+---------+     +--------+---------+
         |                        |                        |
         v                        v                        v
+--------+---------+     +--------+---------+     +--------+---------+
| ggml-cpu library |     | ggml-cuda library|     |ggml-vulkan lib   |
+--------+---------+     +--------+---------+     +--------+---------+
         \                        |                       /
          \                       |                      /
           +----------------------+---------------------+
                                  |
                          +-------v--------+
                          |  ggml-base     |
                          |  (core tensor  |
                          |   operations)  |
                          +-------+--------+
                                  |
                          +-------v--------+
                          |   llama.cpp    |
                          |  (model logic) |
                          +----------------+
```

### Feature Flags

Feature flags are defined in `infrastructure/llama-bindings/Cargo.toml` and propagated through `infrastructure/llama-cpp/Cargo.toml`:

| Feature | CMake Define | Description |
|---------|-------------|-------------|
| `cuda` | `GGML_CUDA=ON` | NVIDIA GPU via CUDA |
| `cuda-no-vmm` | `GGML_CUDA_NO_VMM=ON` | CUDA without virtual memory management (avoids linking `libcuda.so`) |
| `metal` | (implicit on macOS) | Apple Metal GPU acceleration |
| `vulkan` | `GGML_VULKAN=ON` | Cross-platform GPU via Vulkan |
| `openmp` | `GGML_OPENMP=ON` | Multi-threaded CPU via OpenMP |
| `dynamic-link` | `BUILD_SHARED_LIBS=ON` | Build shared libraries instead of static |
| `system-ggml` | `LLAMA_USE_SYSTEM_GGML=ON` | Link against system-installed GGML |
| `mtmd` | `LLAMA_BUILD_TOOLS=ON` | Multimodal support (text/image/audio) |

## Device Discovery

At runtime, the available backend devices can be enumerated:

```rust
use infrastructure_llama_cpp::list_llama_ggml_backend_devices;

let devices = list_llama_ggml_backend_devices();
for (i, dev) in devices.iter().enumerate() {
    println!("Device {i:>2}: {}", dev.name);
    println!("           Description: {}", dev.description);
    println!("           Device Type: {:?}", dev.device_type);
    println!("           Backend: {}", dev.backend);
    println!("           Memory total: {:?} MiB", dev.memory_total / 1024 / 1024);
    println!("           Memory free:  {:?} MiB", dev.memory_free / 1024 / 1024);
}
```

The `LlamaBackendDevice` struct returned contains:

```rust
pub struct LlamaBackendDevice {
    pub name: String,
    pub description: String,
    pub device_type: ggml_backend_dev_type,
    pub backend: String,
    pub memory_total: usize,
    pub memory_free: usize,
}
```

### Backend Capability Queries

```rust
let backend = LlamaBackend::init()?;

// Was the code compiled with GPU support and is a GPU available?
if backend.supports_gpu_offload() {
    println!("GPU offload available");
}

// Can models be memory-mapped?
if backend.supports_mmap() {
    println!("mmap supported");
}

// Can model memory be locked in RAM?
if backend.supports_mlock() {
    println!("mlock supported");
}
```

## CPU Backend

The CPU backend is always compiled. Its performance depends heavily on SIMD instruction support.

### SIMD Feature Detection

The build system (`build.rs`) reads `CARGO_CFG_TARGET_FEATURE` and maps Rust CPU features to GGML CMake defines:

| Rust target feature | GGML CMake define | Architecture |
|---------------------|-------------------|--------------|
| `avx` | `GGML_AVX=ON` | x86_64 |
| `avx2` | `GGML_AVX2=ON` | x86_64 |
| `avx512bf16` | `GGML_AVX512_BF16=ON` | x86_64 |
| `avx512vbmi` | `GGML_AVX512_VBMI=ON` | x86_64 |
| `avx512vnni` | `GGML_AVX512_VNNI=ON` | x86_64 |
| `avxvnni` | `GGML_AVX_VNNI=ON` | x86_64 |
| `bmi2` | `GGML_BMI2=ON` | x86_64 |
| `f16c` | `GGML_F16C=ON` | x86_64 |
| `fma` | `GGML_FMA=ON` | x86_64 |
| `sse4.2` | `GGML_SSE42=ON` | x86_64 |

#### Native Compilation

When `target-cpu=native` is set in `CARGO_ENCODED_RUSTFLAGS`, the build system enables `GGML_NATIVE=ON`, which lets the C compiler auto-detect all available CPU features:

```rust
// From build.rs
if target_cpu == Some("native".into()) {
    config.define("GGML_NATIVE", "ON");
}
```

Build with native CPU support:

```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

#### ARM64 (AArch64) on Linux

For non-native ARM64 builds (common in Docker), the build system explicitly sets the architecture:

```rust
// From build.rs
if matches!(target_os, TargetOs::Linux)
    && target_triple.contains("aarch64")
    && target_cpu != Some("native".into())
{
    config.define("GGML_NATIVE", "OFF");
    config.define("GGML_CPU_ARM_ARCH", "armv8-a");
}
```

ARM NEON SIMD is the baseline for AArch64 and is always enabled. For Apple Silicon (M1/M2/M3/M4), native compilation unlocks additional features like dot product instructions and half-precision float support.

### Threading Configuration

CPU threading is controlled at context creation:

```rust
let ctx_params = LlamaContextParams::default()
    .with_n_threads(8)        // Threads for token generation (single-token decode)
    .with_n_threads_batch(8); // Threads for prompt processing (batch decode)
```

Guidelines:
- `n_threads`: Set to the number of performance cores. For generation, each step processes one token, so diminishing returns beyond physical core count.
- `n_threads_batch`: Can benefit from more threads since prompt processing is heavily parallel. The examples use `std::thread::available_parallelism()`.

### NUMA Support

For multi-socket systems, NUMA-aware memory allocation improves performance:

```rust
use infrastructure_llama_cpp::llama_backend::NumaStrategy;

let backend = LlamaBackend::init_numa(NumaStrategy::DISTRIBUTE)?;
```

Available NUMA strategies:

```rust
pub enum NumaStrategy {
    DISABLED,    // No NUMA awareness
    DISTRIBUTE,  // Distribute work across NUMA nodes
    ISOLATE,     // Isolate to specific NUMA nodes
    NUMACTL,     // Use numactl settings
    MIRROR,      // Mirror memory across nodes
    COUNT,       // Count of strategies
}
```

### OpenMP

When the `openmp` feature is enabled, GGML uses OpenMP for parallel tensor operations instead of its built-in thread pool:

```toml
# Cargo.toml
[dependencies]
infrastructure_llama_bindings = { path = "...", features = ["openmp"] }
```

On GNU/Linux, this additionally links `libgomp`:

```rust
// From build.rs
if cfg!(feature = "openmp") && target_triple.contains("gnu") {
    println!("cargo:rustc-link-lib=gomp");
}
```

OpenMP is automatically disabled for Android targets regardless of the feature flag:

```rust
// From build.rs
if cfg!(feature = "openmp") && !matches!(target_os, TargetOs::Android) {
    config.define("GGML_OPENMP", "ON");
} else {
    config.define("GGML_OPENMP", "OFF");
}
```

## CUDA Backend (NVIDIA GPU)

### Build Configuration

Enable CUDA with the `cuda` feature:

```toml
[dependencies]
infrastructure_llama_bindings = { path = "...", features = ["cuda"] }
```

The build system uses `find_cuda_helper` to locate CUDA libraries:

```rust
// From build.rs
if cfg!(feature = "cuda") {
    config.define("GGML_CUDA", "ON");

    if cfg!(feature = "cuda-no-vmm") {
        config.define("GGML_CUDA_NO_VMM", "ON");
    }
}
```

### Platform-Specific Linking

**Linux** (static linking preferred):

```rust
println!("cargo:rustc-link-lib=static=cudart_static");
println!("cargo:rustc-link-lib=static=cublas_static");
println!("cargo:rustc-link-lib=static=cublasLt_static");
println!("cargo:rustc-link-lib=static=culibos");
if !cfg!(feature = "cuda-no-vmm") {
    println!("cargo:rustc-link-lib=cuda");  // libcuda.so (driver API)
}
```

**Windows** (dynamic linking required because NVIDIA does not ship `culibos.lib`):

```rust
println!("cargo:rustc-link-lib=cudart");    // cudart64_*.dll
println!("cargo:rustc-link-lib=cublas");    // cublas64_*.dll
println!("cargo:rustc-link-lib=cublasLt");  // cublasLt64_*.dll
if !cfg!(feature = "cuda-no-vmm") {
    println!("cargo:rustc-link-lib=cuda");  // nvcuda.dll via cuda.lib
}
```

### cuda-no-vmm Feature

The `cuda-no-vmm` sub-feature disables CUDA Virtual Memory Management. This avoids the need to dynamically link against `libcuda.so` / `cuda.dll` (the driver API), which is useful in environments where:
- The CUDA driver is not available at build time
- You want to reduce runtime dependencies
- Running in containers without full CUDA driver access

### Runtime GPU Configuration

Control how many transformer layers are offloaded to GPU:

```rust
use infrastructure_llama_cpp::model::params::LlamaModelParams;

// Offload all layers to GPU
let model_params = LlamaModelParams::default()
    .with_n_gpu_layers(999);

// Offload specific number of layers (partial offload)
let model_params = LlamaModelParams::default()
    .with_n_gpu_layers(20);  // First 20 layers on GPU, rest on CPU

// CPU only (no GPU offload)
let model_params = LlamaModelParams::default()
    .with_n_gpu_layers(0);
```

### Multi-GPU Configuration

```rust
use infrastructure_llama_cpp::model::params::{LlamaModelParams, LlamaSplitMode};

// Split layers across multiple GPUs (default)
let model_params = LlamaModelParams::default()
    .with_n_gpu_layers(999)
    .with_split_mode(LlamaSplitMode::Layer);

// Use tensor parallelism (row splitting)
let model_params = LlamaModelParams::default()
    .with_n_gpu_layers(999)
    .with_split_mode(LlamaSplitMode::Row);

// Use a single specific GPU
let model_params = LlamaModelParams::default()
    .with_n_gpu_layers(999)
    .with_main_gpu(1)                      // Use GPU index 1
    .with_split_mode(LlamaSplitMode::None); // Single GPU mode

// Select specific GPUs by device index
let model_params = LlamaModelParams::default()
    .with_n_gpu_layers(999)
    .with_devices(&[0, 2])?;  // Use GPU 0 and GPU 2, skip GPU 1
```

Split modes:

```rust
pub enum LlamaSplitMode {
    None = 0,   // Single GPU
    Layer = 1,  // Split layers and KV across GPUs (default)
    Row = 2,    // Split layers and KV across GPUs with tensor parallelism
}
```

### MoE CPU Offloading

For Mixture-of-Experts models (e.g., Mixtral), expert layers can be kept on CPU while attention layers stay on GPU:

```rust
use std::pin::pin;

let mut model_params = pin!(LlamaModelParams::default().with_n_gpu_layers(999));

// Move all MoE expert layers to CPU
model_params.as_mut().add_cpu_moe_override();

// Or specify a custom regex pattern for layers to move to CPU
model_params.as_mut().add_cpu_buft_override(c"\\.ffn_(up|down|gate)_(ch|)exps");
```

The default MoE pattern `\\.ffn_(up|down|gate)_(ch|)exps` matches the feed-forward expert weight matrices in standard MoE architectures.

### KV Cache GPU Offloading

Control whether the KV cache stays on GPU memory:

```rust
let ctx_params = LlamaContextParams::default()
    .with_offload_kqv(true);   // Keep KV cache on GPU (default, faster)

// For GPU VRAM-constrained setups:
let ctx_params = LlamaContextParams::default()
    .with_offload_kqv(false);  // Keep KV cache on CPU (saves VRAM)
```

## Metal Backend (Apple Silicon)

### Automatic Detection

On macOS, Metal is automatically enabled. The `infrastructure/llama-cpp/Cargo.toml` handles this:

```toml
[target.'cfg(target_os = "macos")'.dependencies]
infrastructure_llama_bindings = { path = "../llama-bindings", features = ["metal"] }
```

No explicit feature flag is needed when building on macOS -- Metal support is built in by default.

### macOS Linking

The build system links the required Apple frameworks:

```rust
// From build.rs - TargetOs::Apple
println!("cargo:rustc-link-lib=framework=Foundation");
println!("cargo:rustc-link-lib=framework=Metal");
println!("cargo:rustc-link-lib=framework=MetalKit");
println!("cargo:rustc-link-lib=framework=Accelerate");
println!("cargo:rustc-link-lib=c++");
```

For older macOS versions, the clang runtime library path is discovered and linked:

```rust
// From build.rs
if let Some(path) = macos_link_search_path() {
    println!("cargo:rustc-link-lib=clang_rt.osx");
    println!("cargo:rustc-link-search={}", path);
}
```

The `macos_link_search_path()` function runs `clang --print-search-dirs` to find the correct library path.

### Metal Runtime Configuration

Metal uses the same `n_gpu_layers` parameter as CUDA:

```rust
let model_params = LlamaModelParams::default()
    .with_n_gpu_layers(999);  // Offload everything to Metal GPU
```

Apple Silicon's unified memory architecture means that "GPU offload" does not involve a physical memory copy -- the GPU accesses the same memory, making offloading essentially free.

### BLAS on macOS

The build system explicitly disables BLAS on macOS since Metal's compute shaders handle matrix operations:

```rust
// From build.rs
if matches!(target_os, TargetOs::Apple(_)) {
    config.define("GGML_BLAS", "OFF");
}
```

## Vulkan Backend (Cross-Platform GPU)

### Build Configuration

Enable Vulkan with the `vulkan` feature:

```toml
[dependencies]
infrastructure_llama_bindings = { path = "...", features = ["vulkan"] }
```

### Platform-Specific Setup

**Linux**:

```rust
// From build.rs - TargetOs::Linux with vulkan feature
if let Ok(vulkan_path) = env::var("VULKAN_SDK") {
    let vulkan_lib_path = Path::new(&vulkan_path).join("lib");
    println!("cargo:rustc-link-search={}", vulkan_lib_path.display());
}
println!("cargo:rustc-link-lib=vulkan");
```

If `VULKAN_SDK` is not set, the system's Vulkan libraries (from the system package manager) are used.

**Windows**:

```rust
// From build.rs - TargetOs::Windows with vulkan feature
let vulkan_path = env::var("VULKAN_SDK").expect(
    "Please install Vulkan SDK and ensure that VULKAN_SDK env variable is set",
);
let vulkan_lib_path = Path::new(&vulkan_path).join("Lib");
println!("cargo:rustc-link-search={}", vulkan_lib_path.display());
println!("cargo:rustc-link-lib=vulkan-1");
```

On Windows, the `VULKAN_SDK` environment variable is required. Additionally, the build system works around MSBuild path length issues:

```rust
// From build.rs
env::set_var("TrackFileAccess", "false");
config.cflag("/FS");   // Serialize PDB access
config.cxxflag("/FS");
```

### Runtime Usage

Vulkan uses the same GPU offloading API as CUDA:

```rust
let model_params = LlamaModelParams::default()
    .with_n_gpu_layers(999);  // Offload to Vulkan GPU
```

Vulkan is particularly useful for:
- AMD GPUs on Linux and Windows
- Intel integrated and discrete GPUs
- Cross-platform GPU acceleration without CUDA dependency
- Environments where CUDA is not available

## Android NDK

### Build Configuration

Android builds require the NDK and are detected by the target triple:

```rust
// From build.rs
if target.contains("android")
    || target == "aarch64-linux-android"
    || target == "armv7-linux-androideabi"
    || target == "i686-linux-android"
    || target == "x86_64-linux-android"
{
    Ok((TargetOs::Android, target))
}
```

### NDK Discovery

The build system searches for the Android NDK in order:

1. `ANDROID_NDK` environment variable
2. `ANDROID_NDK_ROOT` environment variable
3. `NDK_ROOT` environment variable
4. `CARGO_NDK_ANDROID_NDK` (set by `cargo ndk`)
5. Auto-detect from `ANDROID_HOME` or `ANDROID_SDK_ROOT` (latest version in `ndk/` subdirectory)

### ABI and Architecture Mapping

| Rust target | Android ABI | Compiler flags |
|------------|-------------|----------------|
| `aarch64-linux-android` | `arm64-v8a` | `-march=armv8-a` |
| `armv7-linux-androideabi` | `armeabi-v7a` | `-march=armv7-a -mfpu=neon -mthumb` |
| `x86_64-linux-android` | `x86_64` | `-march=x86-64` |
| `i686-linux-android` | `x86` | `-march=i686` |

### Android-Specific Configuration

```rust
// From build.rs - Android target
config.define("CMAKE_TOOLCHAIN_FILE", &toolchain_file);
config.define("ANDROID_PLATFORM", &android_platform);  // e.g., "android-28"
config.define("ANDROID_ABI", android_abi);
config.define("GGML_LLAMAFILE", "OFF");  // LlamaFile not supported on Android

// Link Android system libraries
println!("cargo:rustc-link-lib=log");
println!("cargo:rustc-link-lib=android");
```

OpenMP is always disabled on Android, and the `shared-stdcxx` feature controls whether `libc++_shared.so` is dynamically linked.

## Windows MSVC

### Bindgen Header Discovery

The build system uses the `cc` crate to discover MSVC include paths, working around bindgen's inability to find them natively:

```rust
// From build.rs
let compiler = build.try_get_compiler().unwrap();
let env_include = compiler.env().iter()
    .find(|(k, _)| k.eq_ignore_ascii_case("INCLUDE"))
    .map(|(_, v)| v);

if let Some(include_paths) = env_include {
    for include_path in include_paths.to_string_lossy().split(';') {
        bindings_builder = bindings_builder
            .clang_arg("-isystem")
            .clang_arg(include_path);
    }
}
```

### MSVC Release Optimization

A workaround addresses a bug where Rust debug builds strip optimization flags from CMake Release builds:

```rust
// From build.rs
if matches!(target_os, TargetOs::Windows(WindowsVariant::Msvc))
    && matches!(profile.as_str(), "Release" | "RelWithDebInfo" | "MinSizeRel")
{
    for flag in &["/O2", "/DNDEBUG", "/Ob2"] {
        config.cflag(flag);
        config.cxxflag(flag);
    }
}
```

### Windows Linking

```rust
// From build.rs
println!("cargo:rustc-link-lib=advapi32");
if cfg!(debug_assertions) {
    println!("cargo:rustc-link-lib=dylib=msvcrtd");
}
```

## Build System Details

### CMake Configuration

The build system uses the `cmake` crate to compile llama.cpp. Key configuration:

```rust
// From build.rs
let mut config = Config::new(&llama_src);

config.define("LLAMA_BUILD_TESTS", "OFF");
config.define("LLAMA_BUILD_EXAMPLES", "OFF");
config.define("LLAMA_BUILD_TOOLS", "OFF");  // ON when mtmd feature enabled
config.define("LLAMA_CURL", "OFF");
config.define("LLAMA_BUILD_COMMON", "ON");
```

### Parallel Compilation

The build system uses all available CPU cores for compilation:

```rust
env::set_var(
    "CMAKE_BUILD_PARALLEL_LEVEL",
    std::thread::available_parallelism().unwrap().get().to_string(),
);
```

### Corrupted Build State Recovery

The build system detects and recovers from interrupted CMake configurations:

```rust
// From build.rs
let cmake_cache = cmake_build_dir.join("CMakeCache.txt");
let makefile = cmake_build_dir.join("Makefile");
let build_ninja = cmake_build_dir.join("build.ninja");

if cmake_cache.exists() && !makefile.exists() && !build_ninja.exists() {
    std::fs::remove_dir_all(&cmake_build_dir)?;
}
```

### Library Extraction and Linking

After CMake builds, the libraries are extracted and linked:

```rust
// Static or dynamic linking based on feature flags
let llama_libs_kind = if build_shared_libs || cfg!(feature = "system-ggml") {
    "dylib"
} else {
    "static"
};

let llama_libs = extract_lib_names(&out_dir, build_shared_libs);
for lib in llama_libs {
    println!("cargo:rustc-link-lib={}={}", llama_libs_kind, lib);
}
```

For dynamic linking, shared libraries (`.dll`, `.so`, `.dylib`) are hard-linked to the cargo target directory, the `examples/` subdirectory, and `deps/` for test discovery.

### System GGML

When the `system-ggml` feature is enabled, the build system links against pre-installed GGML libraries instead of compiling from source:

```rust
if cfg!(feature = "system-ggml") {
    println!("cargo:rustc-link-lib={llama_libs_kind}=ggml");
    println!("cargo:rustc-link-lib={llama_libs_kind}=ggml-base");
    println!("cargo:rustc-link-lib={llama_libs_kind}=ggml-cpu");
}
```

Library paths are extracted from `CMakeCache.txt` to find where the system GGML was installed.

## Performance Tuning Summary

| Parameter | CPU Impact | GPU Impact |
|-----------|-----------|------------|
| `n_gpu_layers` | Fewer layers on CPU = faster | More layers = more VRAM used |
| `n_threads` | Set to physical core count | Minimal impact (GPU does compute) |
| `n_threads_batch` | More threads = faster prefill | Minimal impact |
| `n_batch` | Larger = more memory, faster prefill | Same |
| `n_ubatch` | Controls physical batch granularity | Same |
| `offload_kqv` | N/A | `true` = faster, uses more VRAM |
| `use_mmap` | Faster model loading | N/A |
| `use_mlock` | Prevents swapping, steady performance | N/A |
| KV cache type | Lower precision = less memory | Same |
| `flash_attention` | Marginal benefit | Significant speedup + memory savings |
| `split_mode` | N/A | `Row` enables tensor parallelism across GPUs |

### Recommended Configurations

**Maximum performance (dedicated GPU)**:
```rust
let model_params = LlamaModelParams::default()
    .with_n_gpu_layers(999);

let ctx_params = LlamaContextParams::default()
    .with_n_ctx(NonZeroU32::new(4096))
    .with_offload_kqv(true)
    .with_flash_attention_policy(flash_attn_type)
    .with_n_threads(1)            // GPU does the work
    .with_n_threads_batch(1);
```

**CPU-only (maximize throughput)**:
```rust
let model_params = LlamaModelParams::default()
    .with_n_gpu_layers(0);

let ctx_params = LlamaContextParams::default()
    .with_n_ctx(NonZeroU32::new(4096))
    .with_n_threads(num_physical_cores)
    .with_n_threads_batch(num_total_cores);
```

**Memory-constrained GPU**:
```rust
let model_params = LlamaModelParams::default()
    .with_n_gpu_layers(20);  // Partial offload

let ctx_params = LlamaContextParams::default()
    .with_n_ctx(NonZeroU32::new(2048))        // Smaller context
    .with_offload_kqv(false)                   // KV cache on CPU
    .with_type_k(KvCacheType::Q8_0)           // Quantized KV cache
    .with_type_v(KvCacheType::Q8_0);
```

See [03-rust-ffi-bindings.md](./03-rust-ffi-bindings.md) for the complete build system analysis and [10-usecases-and-patterns.md](./10-usecases-and-patterns.md) for practical recipes using these configurations.
