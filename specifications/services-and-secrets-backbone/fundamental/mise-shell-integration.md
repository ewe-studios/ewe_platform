# Exploration: mise — Shell Integration, Directory Change Detection, & Env Management

**Source:** Jeff Dickey / @jdx ([github.com/jdx/mise](https://github.com/jdx/mise))
**Crate:** `mise` v2026.4.25 · MIT · Cross-platform (Linux, macOS, Windows)
**Tagline:** "Dev tools, env vars, and tasks in one CLI"

Sources:  /home/darkvoid/Boxxed/@formulas/src.rust/src.mise/
---

## Summary

mise-enplace is a comprehensive developer environment manager that prepares the shell before each command runs. It combines the functionality of `asdf` (tool version management), `direnv` (per-directory env vars), and a task runner into a single CLI. The core mechanism is a **shell hook that fires on every prompt** (`hook-env`), which detects directory changes, config file modifications, and environment drift — then emits minimal shell code to sync the shell state.

Unlike `direnv` which blocks on `cd`, mise uses a **non-blocking session model** — the hook computes a compressed snapshot of the current state and passes it through an environment variable (`__MISE_SESSION`) so subsequent invocations can do a fast-path check without reloading config.

---

## Core Architecture

### Activation Model

```
User runs: eval "$(mise activate zsh)"
    │
    ▼
mise emits shell code that:
    1. Installs a precmd hook (fires before each prompt)
    2. Runs `mise hook-env` on first activation
    3. Stores session data in __MISE_SESSION env var
    │
    ▼
On each prompt (precmd):
    mise hook-env --reason=precmd
    ├── Fast-path check (no config load needed)
    │   ├── Did CWD change?
    │   ├── Were MISE_ env vars modified?
    │   ├── Were config files modified? (stat)
    │   └── Was data dir modified? (stat)
    ├── If fast-path says "no changes" → exit immediately
    └── Otherwise → full hook-env run
        ├── Load config (recursive directory search)
        ├── Compute env diff (old → new)
        ├── Compute tool changes (installed/removed)
        ├── Build new session snapshot
        ├── Serialize session → __MISE_SESSION (gzip+msgpack+base64)
        └── Emit shell code: set/unset env vars, PATH changes, aliases
```

### Key Files

| File | Role |
|------|------|
| `src/hook_env.rs` | Session model, fast-path early exit, file watching, session serialization |
| `src/env_diff.rs` | `EnvDiff` — compute old→new env diffs, bash script parsing, serialization |
| `src/shell/mod.rs` | `Shell` trait — activate/deactivate, set_env, prepend_env per shell |
| `src/shell/zsh.rs` | Zsh-specific shell code generation |
| `src/shell/bash.rs` | Bash-specific shell code generation |
| `src/shell/fish.rs` | Fish-specific shell code generation (with PATH management quirks) |
| `src/hooks.rs` | Lifecycle hooks (Enter, Leave, Cd, Preinstall, Postinstall) |
| `src/direnv.rs` | Direnv integration — parse `.envrc` files |
| `src/cli/hook_env.rs` | The `mise hook-env` CLI command |
| `src/cli/activate.rs` | The `mise activate <shell>` CLI command |

---

## Detailed Mechanisms

### 1. Shell Activation (`shell/*.rs`)

Each shell has an `activate()` method that generates shell-specific code:

**Zsh activation:**
```bash
_mise_hook() { eval "$(mise hook-env --reason=precmd)"; }
chpwd_functions+=( _mise_hook )
precmd_functions+=( _mise_hook )
# On activation, also runs:
eval "$(mise hook-env --reason=activate)"
```

**Bash activation** — installs `PROMPT_COMMAND` and `cd` hooks.
**Fish activation** — uses `--on-event fish_prompt` function.
**Nushell, Elvish, Xonsh, PowerShell** — each has native hook integration.

The `Shell` trait provides:
- `activate()` — generate the hook installation code
- `deactivate()` — reverse all env changes (restore pristine state)
- `set_env(k, v)` — emit shell-specific `export K=V` / `set -x K V` / `set -gx K V`
- `prepend_env(k, v)` — prepend to a PATH-like variable
- `prepend_env_move(k, v)` — prepend, but move existing entry to front if already present (avoids duplicates)
- `unset_env(k)` — emit `unset K` / `set -e K`
- `set_alias(name, cmd)` — shell-specific alias syntax

### 2. Session Model (`hook_env.rs`)

The key innovation: **every hook-env run produces a serialized session** passed via `__MISE_SESSION`:

```rust
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct HookEnvSession {
    pub loaded_tools: IndexSet<String>,    // installed tool versions
    pub loaded_configs: IndexSet<PathBuf>, // config files loaded
    pub config_paths: IndexSet<PathBuf>,   // config search paths
    pub env: EnvMap,                       // env vars set by mise
    pub aliases: IndexMap<String, String>, // shell aliases
    pub tera_files: Vec<PathBuf>,          // files read by tera templates
    pub watch_files: Vec<PathBuf>,         // resolved watch file paths
    dir: Option<PathBuf>,                  // current working directory
    env_var_hash: String,                  // hash of MISE_* env vars
    latest_update: u128,                   // max mtime of watched files
}
```

**Serialization** — MessagePack → zlib → base64 (no padding):
```rust
pub fn serialize<T: serde::Serialize>(obj: &T) -> Result<String> {
    let mut gz = ZlibEncoder::new(Vec::new(), Compression::fast());
    gz.write_all(&rmp_serde::to_vec_named(obj)?)?;
    Ok(BASE64_STANDARD_NO_PAD.encode(gz.finish()?))
}
```

The compressed session is typically small (~100-500 bytes) and fits in an env var.

### 3. Fast-Path Early Exit (`should_exit_early_fast`)

Before loading any config or tool state, the hook checks:

1. **No previous session** → must run full hook-env (first activation)
2. **`--force` flag** → must run
3. **First precmd after activation** → must run (catches shell init PATH changes)
4. **Directory changed** → must run (compare `PREV_SESSION.dir` vs current CWD)
5. **MISE_ env vars modified** → must run (hash comparison, no I/O)
6. **chpwd_only + precmd** → skip (stat-free optimization for NFS)
7. **Cache TTL window** → skip (if within `cache_ttl` ms of last full check)
8. **Config files modified** → check mtime vs `PREV_SESSION.latest_update`
9. **Tera files modified** → check mtime
10. **Watch files modified** → check mtime
11. **Data dir modified** → check mtime (new tools installed)
12. **Config search dirs modified** → check mtime (new config file created)

If all checks pass → exit immediately without loading config. This is critical for performance: a prompt that fires on every keystroke return must be sub-millisecond when nothing changed.

### 4. EnvDiff Computation (`env_diff.rs`)

The `EnvDiff` tracks what changed between two env states:

```rust
pub struct EnvDiff {
    pub old: IndexMap<String, String>,  // keys that existed before
    pub new: IndexMap<String, String>,  // keys after change
    pub path: Vec<PathBuf>,             // PATH entries added by mise
}
```

**Operations** computed as patches:
- `Add(key, value)` — new key not in old env
- `Change(key, new_value)` — key existed but value changed
- `Remove(key)` — key existed in old but not in new

**`from_bash_script()`** — the most interesting method. To capture env changes from a shell script (like `activate` scripts from Python venvs, nvm, etc.):

1. Source the script in a subprocess: `bash --noprofile -c '. "script"; export -p'`
2. Parse `declare -x KEY=VALUE` output
3. Compare against the original env
4. Compute the diff

This allows mise to integrate with any tool that modifies the environment via a shell script.

**`reverse()`** — swaps old and new, used for deactivation to undo all env changes.

**`to_patches()`** — converts the diff into shell-executable operations.

### 5. Directory Change Detection

The `dir_change()` function compares the previous session's directory with the current CWD:

```rust
pub fn dir_change() -> Option<(Option<PathBuf>, PathBuf)> {
    match (&PREV_SESSION.dir, &*dirs::CWD) {
        (Some(old), Some(new)) if old != new => Some((Some(old.clone()), new.clone())),
        (None, Some(new)) => Some((None, new.clone())),
        _ => None,
    }
}
```

When a directory change is detected, hooks are scheduled:
- `Hooks::Leave` — fire for the old directory's config root
- `Hooks::Cd` — fire for all configs
- `Hooks::Enter` — fire for the new directory's config root

### 6. Deactivation & PATH Restoration

Deactivating mise must restore the shell to its pre-mise state, but **preserve any user additions** made to PATH during the session:

```rust
fn compute_deactivated_path() -> String {
    // 1. Get current PATH (may include user additions since activation)
    // 2. Get PATH that mise set during last hook-env
    // 3. Get pristine PATH (from before mise activation)
    // 4. For each path, compute: target = current - mise + pristine
    // 5. Filter current PATH keeping only target copies of each path
    // 6. Append any pristine paths not already kept
}
```

The algorithm uses **copy counting** — if a path appears 3 times in current, 1 in mise, and 2 in pristine, the target is max(3-1, 2) = 2 copies. This handles edge cases where the same path appears multiple times.

**Fish shell special case** — Fish has its own PATH management (as a list, not a string), so the deactivation logic excludes PATH from the reverse diff and computes it separately.

### 7. Lifecycle Hooks (`hooks.rs`)

mise supports 5 hook types:

| Hook | When |
|------|------|
| `Enter` | When entering a directory under a config root |
| `Leave` | When leaving a directory that was under a config root |
| `Cd` | On every directory change |
| `Preinstall` | Before installing a tool version |
| `Postinstall` | After installing a tool version |

**Hook definition formats** (in `mise.toml`):
```toml
# Simple script
enter = "echo 'Entering project'"

# Script with specific shell
enter = { script = "echo hello", shell = "bash" }

# Task reference
enter = { task = "setup" }

# Array of hooks
enter = ["echo hello", { task = "setup" }]
```

**Hook execution context** provides env vars:
- `MISE_PROJECT_ROOT` — the config root directory
- `MISE_CONFIG_ROOT` — the config root directory
- `MISE_ORIGINAL_CWD` — the shell's working directory
- `MISE_PREVIOUS_DIR` — previous CWD (on cd/enter/leave)
- `MISE_INSTALLED_TOOLS` — JSON array of tool info (postinstall only)
- `MISE_NO_HOOKS=1` — prevents recursive hook execution

**Directory matching** — Enter hooks only fire when the new CWD is under the config root AND the old CWD was NOT under the same root. Leave hooks fire when leaving the root.

### 8. File Watching

mise watches multiple file categories for changes:

- **Config files** — `mise.toml`, `.mise.toml`, `.tool-versions`, etc.
- **Tera template files** — files read by `read_file()`, `hash_file()`, etc. in config templates
- **Watch files** — explicit `[[watch_files]]` patterns from config
- **Data directory** — `~/.local/share/mise/installs/` (new tool installations)
- **Config search directories** — ancestor directories up to `MISE_CEILING_PATHS`

**Watch file patterns** support:
```toml
[[watch_files]]
path = "package.json"

[[watch_files]]
path = "src/**/*.rs"  # glob patterns
```

The watch file resolution globs patterns to concrete file paths, storing them in the session so the fast-path can stat them directly without loading config.

### 9. Cache TTL Optimization

For slow filesystems (NFS, network drives), mise supports a configurable cache TTL:

```toml
[settings.hook_env]
cache_ttl = "5s"    # skip filesystem checks within this window
chpwd_only = true   # only run on directory changes, not every precmd
```

When `cache_ttl` is active:
- After a full hook-env run, write the timestamp to `~/.local/state/mise/hook-env-checks/<cwd-hash>`
- Subsequent precmd hooks check this timestamp first
- If within the TTL window → skip all filesystem stats
- The timestamp file is keyed by directory hash so multiple shells in different directories don't interfere

### 10. Trust Model

mise has a trust system for config files:

- First time entering a directory with a `mise.toml`, mise asks for trust confirmation
- Trusted configs are recorded in `~/.local/share/mise/trusted-configs/`
- Untrusted configs are not executed (no tool installs, no env setup, no hooks)
- Users can globally trust or ignore specific directories

---

## Key Takeaways for Our Platform

1. **Session-based fast path** — The `__MISE_SESSION` env var approach is brilliant. By serializing the current state into a compressed env var, every subsequent hook invocation can skip config loading entirely if nothing changed. This is sub-millisecond vs. hundreds of milliseconds for a full config load.

2. **mtime-based change detection** — Instead of inotify/fswatch (which can be unreliable across platforms), mise uses modification timestamps. The `latest_update` field in the session captures the max mtime of all watched files, so a single comparison tells you if anything changed.

3. **EnvDiff via bash subprocess** — `from_bash_script()` sources a script in a clean bash subprocess and captures `export -p` output. This is the most reliable way to capture env changes from any shell script, regardless of how it modifies the environment.

4. **PATH copy-counting for deactivation** — The algorithm that counts path occurrences and computes `target = current - mise + pristine` correctly handles edge cases like duplicate paths, user additions during the session, and paths that appear in both mise and pristine.

5. **Cache TTL + chpwd_only for NFS** — The two optimizations work together: `chpwd_only` skips precmd hooks entirely when the directory hasn't changed, and `cache_ttl` adds a time-based window where even directory-changed hooks skip stat operations (relying on the last known state).

6. **Shell trait abstraction** — 7 shell types supported through a single trait. Each shell implements `set_env`, `prepend_env`, `unset_env`, `set_alias` with shell-specific syntax. This makes adding new shells straightforward.

7. **Hook scheduling with drain-and-execute** — Hooks are scheduled by name (`schedule_hook(Hooks::Enter)`) and executed in a batch. This decouples detection from execution and allows multiple directory changes to be coalesced into a single hook run.

8. **`MISE_NO_HOOKS` anti-recursion guard** — Setting this env var before running hooks prevents `mise run` from spawning a shell that re-triggers hooks. Simple and effective.
