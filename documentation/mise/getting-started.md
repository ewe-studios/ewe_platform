# Getting Started with Mise - EWE Platform

This guide explains how to use `mise` for task running, dependency management, and development environment setup in the EWE Platform project.

---

## Table of Contents

1. [What is Mise?](#what-is-mise)
2. [Installation](#installation)
3. [Quick Start](#quick-start)
4. [Task Management](#task-management)
5. [Dependency Management](#dependency-management)
6. [Available Tasks](#available-tasks)
7. [Migration from Makefile](#migration-from-makefile)

---

## What is Mise?

**mise** (formerly known as `rtx`) is a fast, single-binary tool for managing development environments. It serves as a:

- **Task runner** - Replaces Makefiles with a cleaner TOML syntax
- **Version manager** - Manages multiple tool versions (Node.js, Rust, Python, etc.)
- **Dependency manager** - Installs and manages development tools automatically

### Why Mise over Make?

| Feature | Make | Mise |
|---------|------|------|
| Syntax | Makefile (tab-sensitive) | TOML (clean, readable) |
| Cross-platform | Limited (shell-dependent) | Excellent (Windows, macOS, Linux) |
| Dependency management | Manual | Built-in |
| Version management | External (nvm, rustup) | Built-in |
| Task discovery | `make -p` or read Makefile | `mise tasks` |
| Shell integration | Limited | Full (direnv, shells) |

---

## Installation

### Step 1: Install Mise

Choose one of the following methods:

#### Using Homebrew (macOS/Linux)
```bash
brew install mise
```

#### Using Cargo
```bash
cargo install mise
```

#### Using the standalone installer
```bash
curl https://mise.run | sh
```

#### Arch Linux (AUR)
```bash
yay -S mise
```

### Step 2: Configure Your Shell

Add mise to your shell's initialization file:

#### For Bash (~/.bashrc or ~/.bash_profile)
```bash
echo 'eval "$(~/.local/bin/mise activate bash)"' >> ~/.bashrc
```

#### For Zsh (~/.zshrc)
```bash
echo 'eval "$(~/.local/bin/mise activate zsh)"' >> ~/.zshrc
```

#### For Fish (~/.config/fish/config.fish)
```bash
echo '~/.local/bin/mise activate fish | source' >> ~/.config/fish/config.fish
```

### Step 3: Verify Installation

```bash
mise --version
```

---

## Quick Start

Once mise is installed and configured, you can start using it immediately:

```bash
# Navigate to the project
cd /path/to/ewe_platform

# Install all dependencies and tools
mise install

# Run a task
mise run setup

# List all available tasks
mise tasks
```

---

## Task Management

### Running Tasks

```bash
# Run a specific task
mise run <task-name>

# Examples
mise run setup              # Setup development environment
mise run test:all           # Run all tests
mise run build:release      # Build release binaries
mise run quality            # Run code quality checks
```

### Task Aliases

Mise supports aliases, so you can use either format:

```bash
# Both of these are equivalent
mise run test-all
mise run test:all

# Both of these are equivalent
mise run build-release
mise run build:release
```

### Listing Tasks

```bash
# List all available tasks with descriptions
mise tasks

# Filter tasks by prefix
mise tasks | grep test
mise tasks | grep build
```

---

## Dependency Management

### Tools Configuration

Mise manages tools via the `mise.toml` file. The EWE Platform configuration includes:

```toml
[tools]
node = "22"
"cargo:cargo-nextest" = "latest"
"cargo:cargo-audit" = "latest"
"cargo:bacon" = "latest"
```

### Installing Tools

```bash
# Install all configured tools
mise install

# Install a specific tool
mise install node@22

# Install a cargo package
mise install cargo:cargo-nextest
```

### Checking Installed Tools

```bash
# Show all installed tools and versions
mise ls

# Check current environment
mise current
```

### Environment Variables

Mise manages environment variables via the `[env]` section in `mise.toml`:

```toml
[env]
CARGO_TERM_COLOR = "always"
```

---

## Available Tasks

### Setup Tasks

| Task | Description |
|------|-------------|
| `setup` | Install all dev tools and WASM targets |
| `setup:tools` | Install rustfmt, clippy, rust-analyzer, cargo tools |
| `setup:wasm` | Install WASM compilation targets |
| `setup:check` | Verify all installed tools and versions |

### Build Tasks

| Task | Description |
|------|-------------|
| `build:all` | Build all packages (debug) |
| `build:release` | Build all packages (release) |
| `build:wasm` | Build foundation_nostd for WASM |
| `build:demos` | Build demo WASM binaries |
| `build:tests` | Build all WASM integration tests |
| `clean` | Clean build artifacts |

### Testing Tasks

| Task | Description |
|------|-------------|
| `test:all` | Run all tests (unit + integration) |
| `test:unit` | Run only unit tests |
| `test:integration` | Run integration tests |
| `test:quick` | Quick smoke test |
| `test:nostd` | All foundation_nostd tests |
| `test:wasm` | WASM compilation tests |
| `nextest` | Run tests via bacon + nextest |

### Quality Tasks

| Task | Description |
|------|-------------|
| `quality` | Run fmt + clippy + unit tests |
| `verify-all` | Full verification (quality + all tests) |
| `clippy` | Run clippy (zero warnings) |
| `fmt` | Format all code |
| `fmt:check` | Check code formatting |
| `audit` | Security audit |

### Documentation Tasks

| Task | Description |
|------|-------------|
| `doc` | Generate Rust documentation |
| `doc:open` | Generate and open documentation |
| `doc:nostd` | Generate foundation_nostd docs |

### Benchmarking Tasks

| Task | Description |
|------|-------------|
| `bench` | Run all benchmarks |
| `bench:condvar` | Run CondVar benchmarks |
| `bench:nostd` | Run foundation_testing benchmarks |

---

## Migration from Makefile

### Command Mapping

| Make Command | Mise Command |
|--------------|--------------|
| `make setup` | `mise run setup` |
| `make test-all` | `mise run test:all` |
| `make build-release` | `mise run build:release` |
| `make quality` | `mise run quality` |
| `make clean` | `mise run clean` |

### Why the Syntax Changed

- **Colons over dashes**: Mise uses `namespace:task` convention (e.g., `test:all`)
- **Namespace hierarchy**: Tasks are organized by category (`build:`, `test:`, `setup:`)
- **No tab sensitivity**: TOML is whitespace-friendly

---

## Best Practices

### 1. Use Task Dependencies

Tasks can depend on other tasks:

```toml
[tasks.setup]
depends = ["setup:tools", "setup:wasm"]
```

### 2. Leverage Environment Variables

Use `[env]` for consistent environment across tasks:

```toml
[env]
CARGO_TERM_COLOR = "always"
RUSTFLAGS = "-C link-arg=-s"
```

### 3. Use Scripts for Complex Tasks

For complex logic, use inline scripts:

```toml
[tasks."build:tests"]
run = """
#!/usr/bin/env bash
for dir in ./tests/integrations/*/; do
    pkg=$(basename "$dir")
    mise run build:test-directory TEST_PACKAGE="$pkg"
done
"""
```

### 4. Add Descriptions

Always add descriptions for better discoverability:

```toml
[tasks."test:quick"]
description = "Quick smoke test (fast feedback)"
```

---

## Troubleshooting

### "mise: command not found"

Ensure mise is in your PATH:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

### Tasks Not Showing

Verify the `mise.toml` file exists in the project root:

```bash
ls -la mise.toml
```

### Tool Installation Fails

Check if the tool is available:

```bash
mise ls-remote node  # List available Node.js versions
```

### Clear Cache and Reinstall

```bash
mise cache clean
mise install --force
```

---

## Resources

- [Official Mise Documentation](https://mise.jdx.dev/)
- [Mise GitHub Repository](https://github.com/jdx/mise)
- [EWE Platform mise.toml](../../mise.toml)

---

**Next Steps:**
- Read [Task Reference](./task-reference.md) for detailed task descriptions
- Read [Advanced Usage](./advanced-usage.md) for complex configurations
