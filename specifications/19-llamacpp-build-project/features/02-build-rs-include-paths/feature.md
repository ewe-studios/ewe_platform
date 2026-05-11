# Feature: build-rs-include-paths

## Description

Add the llama.cpp root directory as a `-I` include path in `build.rs` so that wrapper headers can include files relative to the llama.cpp root, rather than relying on a sibling `llama.cpp/` directory.

## Current Code (around line 308-311 of build.rs)

```rust
let mut bindings_builder = bindgen::Builder::default()
    .header("wrapper.h")
    .clang_arg(format!("-I{}", llama_src.join("include").display()))
    .clang_arg(format!("-I{}", llama_src.join("ggml/include").display()))
```

## Target Code

```rust
let mut bindings_builder = bindgen::Builder::default()
    .header("wrapper.h")
    .clang_arg(format!("-I{}", llama_src.display()))
    .clang_arg(format!("-I{}", llama_src.join("include").display()))
    .clang_arg(format!("-I{}", llama_src.join("ggml/include").display()))
```

## Why This Order Matters

The root `-I` path is added first so that `#include "include/llama.h"` and `#include "tools/mtmd/mtmd.h"` resolve correctly. The more specific paths (`include/` and `ggml/include/`) remain for any includes that are relative to those subdirectories (e.g., `#include "llama.h"` without the `include/` prefix, which may appear in llama.cpp's own internal headers).

## Verification

- bindgen invocation completes without "file not found" errors for header includes
- The generated bindings file in `target/` contains expected function and type declarations
