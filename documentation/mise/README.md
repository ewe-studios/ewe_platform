# Mise Documentation - EWE Platform

Welcome to the mise documentation for the EWE Platform project.

---

## Overview

This project has migrated from Makefile-based task running to **mise**, a modern task runner and version manager. Mise provides:

- Cleaner TOML-based configuration
- Better cross-platform support
- Built-in dependency and version management
- Faster task execution

---

## Documentation Index

### For New Users

1. **[Getting Started](./getting-started.md)** - Start here!
   - What is mise?
   - Installation instructions
   - Quick start guide
   - Basic task management

### For Experienced Users

2. **[Task Reference](./task-reference.md)** - Complete task catalog
   - All available tasks by category
   - Detailed task descriptions
   - Command mappings from Makefile

3. **[Advanced Usage](./advanced-usage.md)** - Power user features
   - Task configuration syntax
   - Dependencies and conditions
   - IDE integration
   - CI/CD integration

---

## Quick Commands

```bash
# First time setup
mise install
mise run setup

# Daily development
mise run test:quick      # Quick test feedback
mise run fmt             # Format code
mise run clippy          # Lint checks
mise run build:release   # Release build

# Full verification
mise run verify-all      # Complete CI check
```

---

## Task Categories

| Category | Prefix | Examples |
|----------|--------|----------|
| Setup | `setup:*` | `setup`, `setup:tools`, `setup:wasm` |
| Build | `build:*` | `build:all`, `build:release`, `build:wasm` |
| Test | `test:*` | `test:all`, `test:unit`, `test:wasm` |
| Quality | `quality`, `clippy`, `fmt`, `audit` |
| Docs | `doc:*` | `doc`, `doc:open`, `doc:nostd` |
| Bench | `bench:*` | `bench`, `bench:condvar` |
| Git | `git:*` | `git:update-submodules` |

---

## Migration Status

This project has completed migration from Makefile to mise. All Makefile tasks have been converted to mise tasks with improved organization and naming conventions.

**Old Makefile Command** → **New Mise Command**

| Make | Mise |
|------|------|
| `make setup` | `mise run setup` |
| `make test-all` | `mise run test:all` |
| `make build-release` | `mise run build:release` |
| `make quality` | `mise run quality` |
| `make fmt` | `mise run fmt` |
| `make bench` | `mise run bench` |

---

## Configuration Files

| File | Purpose |
|------|---------|
| `mise.toml` | Main configuration (tasks, tools, environment) |
| `.tool-versions` | (Optional) Legacy tool version pins |
| `rust-toolchain.toml` | Rust toolchain version (managed by rustup) |

---

## Related Documentation

- [Foundation NoStd Documentation](../foundation_nostd/doc.md)
- [Foundation Core Documentation](../foundation_core/doc.md)
- [Project README](../../README.md)

---

## Need Help?

- Run `mise tasks` to list all available tasks
- Run `mise help <task>` for task-specific help
- Check the [official mise documentation](https://mise.jdx.dev/)
