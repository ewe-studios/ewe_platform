# Feature: submodule-relocation

## Description

Move the llama.cpp git submodule from `infrastructure/llama-bindings/llama.cpp` to `tools/llama.cpp` and update all configuration files that reference the old path.

## Files to Update

### .gitmodules

**Before:**
```
[submodule "infrastructure/llama-bindings/llama.cpp"]
	path = infrastructure/llama-bindings/llama.cpp
	url = https://github.com/ggml-org/llama.cpp
```

**After:**
```
[submodule "tools/llama.cpp"]
	path = tools/llama.cpp
	url = https://github.com/ggml-org/llama.cpp
```

### mise.toml

Three locations reference the old path:

1. **[env] section (line 15):**
   - Before: `LLAMA_DIR = { value = "infrastructure/llama-bindings/llama.cpp", relative=true, force = true }`
   - After: `LLAMA_DIR = { value = "tools/llama.cpp", relative=true, force = true }`

2. **tasks."git:update-submodules" (line 158):**
   - Before: `git submodule update --remote infrastructure/llama-bindings/llama.cpp/`
   - After: `git submodule update --remote tools/llama.cpp/`

3. **tasks."llama:server:clean" (line 506):**
   - Before: `LLAMA_DIR="$PROJECT_ROOT/infrastructure/llama-bindings/llama.cpp"`
   - After: `LLAMA_DIR="$PROJECT_ROOT/tools/llama.cpp"`

### .cargo/config.toml

**Before (line 15):**
```
LLAMA_DIR = { value = "infrastructure/llama-bindings/llama.cpp", relative=true, force = true }
```

**After:**
```
LLAMA_DIR = { value = "tools/llama.cpp", relative=true, force = true }
```

## Submodule Move Steps

1. `git submodule deinit -f infrastructure/llama-bindings/llama.cpp`
2. `git rm infrastructure/llama-bindings/llama.cpp`
3. `git submodule add https://github.com/ggml-org/llama.cpp tools/llama.cpp`
4. `git add .gitmodules`
5. Update config files as listed above
6. Commit all changes

## Verification

- `git submodule status` shows `tools/llama.cpp` with a commit hash
- `git submodule update --remote tools/llama.cpp` works
- No references to `infrastructure/llama-bindings/llama.cpp` remain in the codebase (verified via grep)
