# Feature: submodule-relocation

## Description

Move the llama.cpp git submodule from `infrastructure/llama-bindings/llama.cpp` to `tools/llama.cpp`, create a symlink for cargo publish compatibility, and update all configuration files and mise tasks.

## mise.toml — Single Source of Truth Variables

Add to `[env]` section, right after `PROJECT_ROOT`:

```toml
TOOLS_DIR = "{{config_root}}/tools"
DEPOT_DIR = "{{env.TOOLS_DIR}}/depot_tools"
DAWN_DIR = "{{env.TOOLS_DIR}}/dawn"
EMSDK_DIR = "{{env.TOOLS_DIR}}/emsdk"
WHISPER_DIR = "{{env.TOOLS_DIR}}/whisper.cpp"
LLAMA_SUBMODULE_PATH = "tools/llama.cpp"
LLAMA_DIR = "{{config_root}}/{{env.LLAMA_SUBMODULE_PATH}}"
LLAMA_BINDINGS_DIR = "{{config_root}}/infrastructure/llama-bindings"
LLAMA_SYMLINK_DIR = "{{env.LLAMA_BINDINGS_DIR}}/llama.cpp"
```

This covers all `tools/` subdirectories with a consistent naming convention. If any tool path changes, only the one variable needs updating.

## .cargo/config.toml — Variable-Driven Paths

The `.cargo/config.toml` cannot use mise `{{env.*}}` interpolation — cargo resolves its own `[env]` section. But by keeping the variable name (`LLAMA_DIR`) consistent between mise.toml and `.cargo/config.toml`, there is a single logical source of truth.

**Before (line 15):**
```toml
LLAMA_DIR = { value = "infrastructure/llama-bindings/llama.cpp", relative = true, force = true }
```

**After:**
```toml
LLAMA_DIR = { value = "tools/llama.cpp", relative = true, force = true }
```

Other entries (`TOOLS_DIR`, `DAWN_DIR`, `EMSDK_DIR`, `LLAMA_BINDINGS_DIR`) remain unchanged — they already point to the correct `tools/` paths.

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

### mise.toml — Updated Tasks

All tasks now use the env variables instead of hardcoding paths:

1. **tasks."tools:llama:symlink"** — creates the symlink for cargo publish (idempotent)

2. **tasks."tools:llama:init"** — initializes and syncs the llama.cpp submodule, depends on symlink task

3. **tasks."tools:emsdk:init"** — initializes and syncs the emsdk submodule

4. **tasks."setup:tools"** — gains `tools:llama:init`, `tools:emsdk:init`, and `tools:dawn:clone` as dependencies so `mise run setup` pulls all C/C++ sources in the correct order

5. **tasks."git:update-submodules"** — uses `{{env.LLAMA_SUBMODULE_PATH}}` for the llama.cpp entry

6. **tasks."setup:depot-tools"** — uses `$DEPOT_DIR` and `$TOOLS_DIR` variables instead of `$PROJECT_ROOT/tools/depot_tools`

7. **tasks."tools:dawn:clone"** — uses `$DAWN_DIR` and `$TOOLS_DIR` variables instead of `$PROJECT_ROOT/tools/dawn`

8. **tasks."llama:server:clean"** — uses `$LLAMA_DIR` directly from mise `[env]` (no longer redefines it inline)

### .cargo/config.toml

**Before:**
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
5. Create symlink: `infrastructure/llama-bindings/llama.cpp -> ../../tools/llama.cpp` (or run `mise run tools:llama:symlink`)
6. Update config files as listed above
7. Commit all changes

## Verification

- `git submodule status` shows `tools/llama.cpp` with a commit hash
- `git submodule update --remote tools/llama.cpp` works
- `ls -la infrastructure/llama-bindings/llama.cpp` shows symlink to `../../tools/llama.cpp`
- `mise run setup` completes successfully (pulls dawn, emsdk, llama.cpp, creates symlink)
- No hardcoded `infrastructure/llama-bindings/llama.cpp` paths remain in `mise.toml` (verified via grep)
- `mise.toml` tasks use `{{env.LLAMA_*}}` variables instead of inline paths
