# Mise Advanced Usage - EWE Platform

This guide covers advanced mise configurations and usage patterns for the EWE Platform project.

---

## Table of Contents

1. [Task Configuration Syntax](#task-configuration-syntax)
2. [Working Directory](#working-directory)
3. [Task Dependencies](#task-dependencies)
4. [Environment Variables](#environment-variables)
5. [Conditional Execution](#conditional-execution)
6. [Integration with IDEs](#integration-with-ides)
7. [Custom Tasks](#custom-tasks)

---

## Task Configuration Syntax

### Basic Task

```toml
[tasks.hello]
description = "Print hello world"
run = "echo 'Hello, World!'"
```

### Multi-line Scripts

```toml
[tasks."build:complex"]
description = "Complex build with multiple steps"
run = """
echo "Starting build..."
cargo build --release
cargo build --target wasm32-unknown-unknown
echo "Build complete!"
"""
```

### Bash Scripts with Arguments

```toml
[tasks."test:single"]
description = "Run a single test file"
run = """
#!/usr/bin/env bash
TEST_NAME="${1:-all}"
cargo test --package foundation_nostd "$TEST_NAME"
"""
```

Usage:
```bash
mise run test:single my_test_function
```

---

## Working Directory

Tasks can specify a working directory:

```toml
[tasks."examples:todo:serve"]
description = "Serve todo example"
dir = "examples/todo"
run = "localserver ./web 8080"
```

This is useful for:
- Running commands in specific subdirectories
- Example-specific tooling (npm, tailwindcss)
- Isolated build processes

---

## Task Dependencies

### Declaring Dependencies

Tasks can depend on other tasks:

```toml
[tasks.setup]
description = "Full setup"
depends = ["setup:tools", "setup:wasm"]
run = 'echo "Setup complete"'
```

### Dependency Execution Order

When you run `mise run setup`:
1. `setup:tools` runs first
2. `setup:wasm` runs second
3. `setup` runs last

### Multiple Dependencies

```toml
[tasks.quality]
depends = ["fmt:check", "clippy", "test:unit"]
run = 'echo "All quality checks passed"'
```

All dependencies run in parallel (where possible) before the main task.

---

## Environment Variables

### Global Environment

Set environment variables for all tasks:

```toml
[env]
CARGO_TERM_COLOR = "always"
RUSTFLAGS = "-C link-arg=-s"
NODE_ENV = "development"
```

### Task-Specific Environment

```toml
[tasks."build:release"]
description = "Release build"
env = { PROFILE = "release", OPTIMIZE = "true" }
run = """
echo "Building with profile: $PROFILE"
cargo build --$PROFILE
"""
```

### Using .env Files

Create a `.env` file in the project root:

```bash
# .env
DATABASE_URL=postgres://localhost/dev
API_KEY=your-api-key
```

Mise automatically loads `.env` files.

---

## Conditional Execution

### File-based Conditions

```toml
[tasks."build:if-changed"]
description = "Build if sources changed"
sources = ["src/**/*.rs", "Cargo.toml"]
outputs = ["target/release/ewe_platform"]
run = "cargo build --release"
```

### Platform-specific Tasks

```toml
[tasks."setup:macos"]
description = "macOS-specific setup"
os = "macos"
run = "brew install wasm-pack"

[tasks."setup:linux"]
description = "Linux-specific setup"
os = "linux"
run = "sudo apt install wasm-tools"
```

---

## Integration with IDEs

### VS Code

Create `.vscode/tasks.json`:

```json
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "mise: test:all",
      "type": "shell",
      "command": "mise run test:all",
      "problemMatcher": ["$rustc"],
      "group": "test"
    },
    {
      "label": "mise: build:release",
      "type": "shell",
      "command": "mise run build:release",
      "problemMatcher": ["$rustc"],
      "group": "build"
    }
  ]
}
```

### Neovim

With `neotask.nvim` or similar:

```lua
require("neotask").setup({
  tasks = {
    test = function()
      vim.fn.jobstart("mise run test:all")
    end,
    build = function()
      vim.fn.jobstart("mise run build:release")
    end,
  },
})
```

### Rust Analyzer

Mise integrates seamlessly with rust-analyzer. Ensure your `[tools]` section includes:

```toml
[tools]
rust-analyzer = "latest"
```

---

## Custom Tasks

### Adding New Tasks

Add custom tasks to `mise.toml` for your workflow:

```toml
# Personal development tasks
[tasks."dev:my-feature"]
description = "Test my in-progress feature"
run = """
cargo test --package foundation_nostd --lib my_feature
cargo clippy --package ewe_platform -- -W clippy::pedantic
"""
```

### Task Templates

For repetitive tasks, create parameterized templates:

```toml
[tasks."test:package"]
description = "Run tests for a specific package"
run = """
#!/usr/bin/env bash
PACKAGE="${1:-}"
if [ -z "$PACKAGE" ]; then
    echo "Usage: mise run test:package <package-name>"
    exit 1
fi
cargo test --package "$PACKAGE"
"""
```

---

## Performance Tips

### Parallel Task Execution

Mise runs independent dependencies in parallel:

```toml
[tasks."ci:all"]
depends = ["quality", "test:all", "build:release"]
```

All three dependencies start simultaneously.

### Caching

Use `sources` and `outputs` for incremental builds:

```toml
[tasks."build:incremental"]
sources = ["src/**/*.rs"]
outputs = ["target/debug/ewe_platform"]
run = "cargo build"
```

### Preloading Tools

Keep tools pre-loaded for faster execution:

```bash
# Pre-load all tools
mise install
mise use --env
```

---

## CI/CD Integration

### GitHub Actions

```yaml
name: CI
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: jdx/mise-action@v2
      - run: mise run ci:all
```

### GitLab CI

```yaml
stages:
  - test
  - build

test:
  stage: test
  image: rust:latest
  before_script:
    - curl https://mise.run | sh
    - mise install
  script:
    - mise run test:all
```

---

## Troubleshooting Advanced Issues

### Task Not Found

```bash
# Verify task exists
mise tasks | grep my-task

# Check mise.toml syntax
mise doctor
```

### Environment Not Loaded

```bash
# Reload environment
mise env --yes | source

# Or use direnv integration
mise activate direnv
```

### Tool Version Conflicts

```bash
# Check which version is active
mise current

# Override for current session
mise use node@20 --env

# Override permanently for this directory
mise use node@20
```

---

## Resources

- [Mise Task Configuration](https://mise.jdx.dev/tasks/)
- [Mise Environment](https://mise.jdx.dev/environments/)
- [EWE Platform mise.toml](../../mise.toml)
