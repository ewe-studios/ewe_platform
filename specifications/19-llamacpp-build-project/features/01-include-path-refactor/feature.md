# Feature: include-path-refactor

## Description

Update `wrapper.h` and `wrapper_mtmd.h` to use include paths that are relative to the llama.cpp root directory (provided via `-I` flag in build.rs), rather than relying on a `llama.cpp/` sibling directory.

## Current Content

### wrapper.h
```c
#include "llama.cpp/include/llama.h"
```

### wrapper_mtmd.h
```c
#include "llama.cpp/tools/mtmd/mtmd.h"
#include "llama.cpp/tools/mtmd/mtmd-helper.h"
```

## Target Content

### wrapper.h
```c
#include "include/llama.h"
```

### wrapper_mtmd.h
```c
#include "tools/mtmd/mtmd.h"
#include "tools/mtmd/mtmd-helper.h"
```

## Why This Works

With `build.rs` adding `-I {llama_src}` (the llama.cpp root directory), clang resolves `#include "include/llama.h"` by searching `{llama_src}/include/llama.h`, which is the correct file. Similarly, `#include "tools/mtmd/mtmd.h"` resolves to `{llama_src}/tools/mtmd/mtmd.h`.

## Dependencies

- **build-rs-include-paths**: The `-I` root path must be added in build.rs first (or in the same PR), otherwise the new includes will not resolve.

## Verification

- `cargo check -p infrastructure_llama_bindings` succeeds
- bindgen generates bindings for `llama_.*` and `ggml_.*` symbols
- With `--features mtmd`, bindgen also generates `mtmd_.*` symbols
