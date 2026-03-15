# 03 - Rust FFI Bindings Layer

The FFI bindings layer (`infrastructure/llama-bindings/`) is the foundation of the integration. It uses `bindgen` to generate Rust declarations from llama.cpp's C headers and `cmake` to compile the C++ source code.

**Crate name**: `infrastructure_llama_bindings`
**Source location**: `infrastructure/llama-bindings/`

## Architecture

```
infrastructure/llama-bindings/
  build.rs           -- 1000+ line build script
  wrapper.h          -- #include "llama.cpp/include/llama.h"
  wrapper_mtmd.h     -- #include multimodal headers (feature-gated)
  src/lib.rs          -- include!(concat!(env!("OUT_DIR"), "/bindings.rs"))
  Cargo.toml          -- feature flags, build dependencies
  llama.cpp/          -- git submodule of upstream llama.cpp
```

## The `lib.rs` File

The entire Rust source is a single line that includes the bindgen-generated code:

```rust
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unpredictable_function_pointer_comparisons)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
```

All type and function declarations come from the auto-generated `bindings.rs` file, which is produced by bindgen during the build.

## Wrapper Headers

### `wrapper.h`
```c
#include "llama.cpp/include/llama.h"
```

This single include exposes the entire llama.cpp public API, including GGML types.

### `wrapper_mtmd.h` (feature-gated on `mtmd`)
```c
#include "llama.cpp/tools/mtmd/mtmd.h"
#include "llama.cpp/tools/mtmd/mtmd-helper.h"
```

Multi-model multi-device (multimodal) support headers.

## Build Script (`build.rs`)

The build script is the most complex part of the bindings layer at 1000+ lines. It handles:

### 1. Environment Variables

Required environment variables:
- `TOOLS_DIR` - Path to platform tools
- `EMSDK_DIR` - Emscripten SDK path
- `LLAMA_DIR` - Path to llama.cpp source directory
- `DAWN_DIR` - Path to Dawn (WebGPU) directory

Optional environment variables:
- `CMAKE_PREFIX_PATH` - Additional CMake search paths
- `LLAMA_LIB_PROFILE` - Build profile (default: "Release")
- `LLAMA_BUILD_SHARED_LIBS` - "1" to build shared libraries
- `LLAMA_STATIC_CRT` - "1" for static C runtime (Windows)
- `BUILD_DEBUG` - Enable debug logging in build script
- `CMAKE_VERBOSE` - Enable verbose CMake output

### 2. Target OS Detection

```rust
enum TargetOs {
    Windows(WindowsVariant),  // Msvc or Other
    Apple(AppleVariant),      // MacOS or Other (iOS)
    Linux,
    Android,
}
```

The build script parses the Rust target triple to determine the OS and configures platform-specific settings.

### 3. Bindgen Configuration

```rust
let mut bindings_builder = bindgen::Builder::default()
    .header("wrapper.h")
    .clang_arg(format!("-I{}", llama_src.join("include").display()))
    .clang_arg(format!("-I{}", llama_src.join("ggml/include").display()))
    .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
    .derive_partialeq(true)
    .allowlist_function("ggml_.*")
    .allowlist_type("ggml_.*")
    .allowlist_function("llama_.*")
    .allowlist_type("llama_.*")
    .prepend_enum_name(false);
```

Key bindgen settings:
- **Allowlists**: Only `ggml_*` and `llama_*` symbols are generated (filtering out system headers)
- **PartialEq derivation**: Generated types implement `PartialEq`
- **Enum naming**: `prepend_enum_name(false)` prevents redundant enum variant prefixes
- **MTMD feature**: When enabled, adds `mtmd_*` allowlists

### 4. Platform-Specific Bindgen Configuration

#### Android NDK Setup

The build script auto-detects the Android NDK from multiple environment variables:
- `ANDROID_NDK`, `ANDROID_NDK_ROOT`, `NDK_ROOT`, `CARGO_NDK_ANDROID_NDK`
- Falls back to `ANDROID_HOME/ndk/<latest_version>`

It configures:
- Sysroot paths for the target architecture
- Clang builtin include paths
- Android API level defines (`__ANDROID_API__`, `__ANDROID__`)
- Architecture mapping: aarch64 -> `aarch64-linux-android`, armv7 -> `arm-linux-androideabi`

#### Windows MSVC Setup

Uses the `cc` crate to discover MSVC include paths:

```rust
let compiler = build.try_get_compiler().unwrap();
let env_include = compiler.env().iter()
    .find(|(k, _)| k.eq_ignore_ascii_case("INCLUDE"));
```

Adds MSVC compatibility flags: `-fms-compatibility`, `-fms-extensions`.

### 5. CMake Configuration

The build script configures CMake with:

```rust
config.define("LLAMA_BUILD_TESTS", "OFF");
config.define("LLAMA_BUILD_EXAMPLES", "OFF");
config.define("LLAMA_BUILD_TOOLS", "OFF");  // ON if mtmd feature
config.define("LLAMA_CURL", "OFF");
config.define("LLAMA_BUILD_COMMON", "ON");
```

### 6. CPU Feature Detection

The build script reads `CARGO_CFG_TARGET_FEATURE` and maps Rust target features to GGML CMake flags:

| Rust Feature | GGML CMake Flag |
|-------------|-----------------|
| `avx` | `GGML_AVX=ON` |
| `avx2` | `GGML_AVX2=ON` |
| `avx512bf16` | `GGML_AVX512_BF16=ON` |
| `avx512vbmi` | `GGML_AVX512_VBMI=ON` |
| `avx512vnni` | `GGML_AVX512_VNNI=ON` |
| `avxvnni` | `GGML_AVX_VNNI=ON` |
| `bmi2` | `GGML_BMI2=ON` |
| `f16c` | `GGML_F16C=ON` |
| `fma` | `GGML_FMA=ON` |
| `sse4.2` | `GGML_SSE42=ON` |

If `target-cpu=native` is set in RUSTFLAGS, it enables `GGML_NATIVE=ON` instead, letting the compiler auto-detect features.

For Linux aarch64 without native CPU, it sets `GGML_CPU_ARM_ARCH=armv8-a` for Docker compatibility.

### 7. GPU Backend Configuration

#### CUDA

```rust
if cfg!(feature = "cuda") {
    config.define("GGML_CUDA", "ON");
    if cfg!(feature = "cuda-no-vmm") {
        config.define("GGML_CUDA_NO_VMM", "ON");
    }
}
```

Linking:
- **Linux**: Static linking to `cudart_static`, `cublas_static`, `cublasLt_static`, `culibos`, and `cuda` driver
- **Windows**: Dynamic linking to `cudart`, `cublas`, `cublasLt`, and `cuda`
- `cuda-no-vmm` feature skips `cuda` driver linkage (for containers without VMM access)

#### Vulkan

```rust
if cfg!(feature = "vulkan") {
    config.define("GGML_VULKAN", "ON");
}
```

- **Windows**: Links `vulkan-1` from `VULKAN_SDK/Lib`, disables file tracking for long paths
- **Linux**: Links `vulkan`, optionally uses `VULKAN_SDK/lib` path

#### Metal (macOS)

Metal is auto-enabled on macOS via Cargo.toml:

```toml
[target.'cfg(all(target_os = "macos", any(target_arch = "aarch64", target_arch = "arm64")))'.dependencies]
infrastructure_llama_bindings = { workspace = true, features = ["metal"] }
```

Links: `Foundation`, `Metal`, `MetalKit`, `Accelerate` frameworks, and `c++`.

#### OpenMP

```rust
if cfg!(feature = "openmp") && !matches!(target_os, TargetOs::Android) {
    config.define("GGML_OPENMP", "ON");
}
```

OpenMP is disabled on Android regardless of the feature flag. On GNU targets, links `gomp`.

### 8. Linking Strategy

The build script determines static vs dynamic linking:

```rust
let llama_libs_kind = if build_shared_libs || cfg!(feature = "system-ggml") {
    "dylib"
} else {
    "static"
};
```

Library extraction uses glob patterns to find built libraries:
- Linux: `*.a` (static) or `*.so` (shared)
- macOS: `*.a` (static) or `*.dylib` (shared)
- Windows: `*.lib`

Shared libraries are hard-linked to the target directory, `deps/`, and `examples/` for runtime accessibility.

### 9. System Dependencies by Platform

| Platform | System Libraries |
|----------|-----------------|
| Linux | `stdc++` |
| macOS | `Foundation`, `Metal`, `MetalKit`, `Accelerate`, `c++`, optionally `clang_rt.osx` |
| Windows (MSVC) | `advapi32`, optionally `msvcrtd` (debug) |
| Android | `log`, `android` |

### 10. Corrupted Build State Recovery

The build script detects and recovers from interrupted CMake configurations:

```rust
if cmake_cache.exists() && !makefile.exists() && !build_ninja.exists() {
    debug_log!("Detected corrupted CMake state, cleaning build directory");
    std::fs::remove_dir_all(&cmake_build_dir)?;
}
```

## Feature Flags

Defined in `infrastructure/llama-bindings/Cargo.toml`:

```toml
[features]
cuda = []
cuda-no-vmm = ["cuda"]
metal = []
dynamic-link = []
vulkan = []
openmp = []
shared-stdcxx = []    # Android only
system-ggml = []      # Use system-installed GGML
mtmd = []             # Multi-model multi-device (multimodal)
```

Feature propagation in `infrastructure/llama-cpp/Cargo.toml`:

```toml
[features]
default = ["openmp", "android-shared-stdcxx"]
mtmd = ["infrastructure_llama_bindings/mtmd"]
cuda = ["infrastructure_llama_bindings/cuda"]
metal = ["infrastructure_llama_bindings/metal"]
vulkan = ["infrastructure_llama_bindings/vulkan"]
openmp = ["infrastructure_llama_bindings/openmp"]
dynamic-link = ["infrastructure_llama_bindings/dynamic-link"]
cuda-no-vmm = ["cuda", "infrastructure_llama_bindings/cuda-no-vmm"]
android-shared-stdcxx = ["infrastructure_llama_bindings/shared-stdcxx"]
system-ggml = ["infrastructure_llama_bindings/system-ggml"]
```

## Rebuild Triggers

The build script monitors these paths for changes:
- `build.rs` itself
- `wrapper.h`, `wrapper_mtmd.h`
- `llama.cpp/src/` - Core source files
- `llama.cpp/ggml/src/` - GGML source files
- `llama.cpp/common/` - Common utilities
- `llama.cpp/tools/` - Tools (including mtmd)
- `llama.cpp/CMakeLists.txt` and sub-project CMake files
- Environment variables: `LLAMA_LIB_PROFILE`, `LLAMA_BUILD_SHARED_LIBS`, `LLAMA_STATIC_CRT`, `CUDA_PATH`
- Hidden directories are excluded from the file walk

## Build Dependencies

```toml
[build-dependencies]
bindgen = { workspace = true }
cc = { workspace = true, features = ["parallel"] }
cmake = "0.1"
find_cuda_helper = "0.2.0"
glob = "0.3.3"
walkdir = "2"
```

## Extending for New llama.cpp Features

To add support for a new llama.cpp feature:

1. **If the feature is in `llama.h`**: It is automatically included in bindgen output. Access it via `infrastructure_llama_bindings::llama_new_function()`.

2. **If it requires a new header**: Add a new `wrapper_*.h` file, add it to `build.rs` bindgen configuration with appropriate allowlists, and gate it behind a feature flag.

3. **If it requires a new CMake option**: Add `config.define("GGML_NEW_FEATURE", "ON")` guarded by `cfg!(feature = "new-feature")` in `build.rs`.

4. **If it requires new system libraries**: Add `println!("cargo:rustc-link-lib=...")` in the appropriate platform section of `build.rs`.

5. **Feature flag**: Add the feature to both `infrastructure/llama-bindings/Cargo.toml` and `infrastructure/llama-cpp/Cargo.toml` with proper propagation.

See [04-rust-safe-wrappers.md](./04-rust-safe-wrappers.md) for the safe wrapper layer that consumes these bindings.
