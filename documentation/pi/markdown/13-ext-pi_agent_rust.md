---
title: "Pi Extensions -- pi_agent_rust"
---

# pi_agent_rust

**High-performance AI coding agent CLI written in Rust.**

pi_agent_rust is a Rust-based implementation of the Pi coding agent CLI, offering significantly better performance than the TypeScript version. It provides the same core functionality -- running LLM-powered coding agents -- with lower memory footprint and faster startup.

## Why It Exists

The default Pi coding agent is written in TypeScript. For users who need maximum performance (large codebases, constrained environments, batch processing), pi_agent_rust provides a drop-in replacement with Rust's speed advantages.

## Key Features

- Rust-native performance -- faster startup, lower memory
- Drop-in compatibility with Pi coding agent workflows
- Same tool execution model and event system

## Quick Start

```bash
# Install from source
cargo install --path .

# Run as coding agent
pi_agent_rust --model claude-sonnet-4-6 "Fix the auth bug"
```

## Package Details

| Property | Value |
|----------|-------|
| Language | Rust |
| Dependencies | Minimal (no Node.js runtime needed) |
| Install | `cargo install` or prebuilt binary |
