# Cross-Platform Configuration Template

This directory contains example `Cargo.toml` configurations for different use cases.

## Template 1: Desktop/Server Application

**Use case**: Standard application that always runs with `std`

```toml
[package]
name = "my-server-app"
version = "0.1.0"
edition = "2021"

[dependencies]
foundation_nostd = { version = "0.0.4", features = ["std"] }

# Your application code uses:
# use foundation_nostd::comp::{Mutex, RwLock};
```

---

## Template 2: Pure Embedded/No-Std Library

**Use case**: Library designed for embedded systems without `std`

```toml
[package]
name = "my-embedded-lib"
version = "0.1.0"
edition = "2021"

[features]
default = []

[dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }

# Optional: Add std support for testing on host
[dev-dependencies]
foundation_nostd = { version = "0.0.4", features = ["std"] }
```

---

## Template 3: Cross-Platform Library (Auto-Detect)

**Use case**: Library that automatically uses `std` on native, `no_std` on WASM

```toml
[package]
name = "my-cross-platform-lib"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["lib"]

[features]
default = []
std = ["foundation_nostd/std"]

[dependencies]
# Base dependency without std
foundation_nostd = { version = "0.0.4", default-features = false }

# Automatically enable std for native platforms
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
foundation_nostd = { version = "0.0.4", features = ["std"] }
```

**Usage**:
```bash
# Native build (automatically uses std)
cargo build

# WASM build (automatically uses no_std)
cargo build --target wasm32-unknown-unknown
```

---

## Template 4: WASM + Native Web Application

**Use case**: Web app that compiles to both WASM (frontend) and native (backend)

```toml
[package]
name = "my-web-app"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[features]
# Default to no_std for WASM compatibility
default = []

# Enable std support (for native backend)
std = ["foundation_nostd/std"]

[dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }

# WASM-specific dependencies (no_std)
[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"

# Native-specific dependencies (with std)
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
tokio = { version = "1", features = ["full"] }
foundation_nostd = { version = "0.0.4", features = ["std"] }
```

**Build commands**:
```bash
# Frontend (WASM, no_std)
wasm-pack build --target web

# Backend (Native, std)
cargo build --features std
```

---

## Template 5: Workspace with Multiple Crates

**Use case**: Workspace with shared and platform-specific crates

### Workspace root `Cargo.toml`:

```toml
[workspace]
members = ["core", "native", "wasm"]
resolver = "2"

[workspace.dependencies]
foundation_nostd = "0.0.4"

[workspace.package]
version = "0.1.0"
edition = "2021"
```

### Core library `core/Cargo.toml`:

```toml
[package]
name = "my-core"
version.workspace = true
edition.workspace = true

[features]
default = []
std = ["foundation_nostd/std"]

[dependencies]
foundation_nostd = { workspace = true, default-features = false }
```

### Native binary `native/Cargo.toml`:

```toml
[package]
name = "my-native"
version.workspace = true
edition.workspace = true

[dependencies]
my-core = { path = "../core", features = ["std"] }
foundation_nostd = { workspace = true, features = ["std"] }
```

### WASM library `wasm/Cargo.toml`:

```toml
[package]
name = "my-wasm"
version.workspace = true
edition.workspace = true

[lib]
crate-type = ["cdylib"]

[dependencies]
my-core = { path = "../core", default-features = false }
foundation_nostd = { workspace = true, default-features = false }
wasm-bindgen = "0.2"
```

---

## Template 6: Library with Optional Std (User Choice)

**Use case**: Library that lets users choose whether to enable `std`

```toml
[package]
name = "my-optional-std-lib"
version = "0.1.0"
edition = "2021"

[features]
# Default includes std for convenience
default = ["std"]

# Optional std support
std = ["foundation_nostd/std"]

[dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }
```

**User options**:
```toml
# User wants std (default)
[dependencies]
my-optional-std-lib = "0.1"

# User wants no_std
[dependencies]
my-optional-std-lib = { version = "0.1", default-features = false }
```

---

## Template 7: Environment-Based Configuration

**Use case**: Different features for development vs. production

```toml
[package]
name = "my-env-config-app"
version = "0.1.0"
edition = "2021"

[features]
default = []
std = ["foundation_nostd/std"]
production = []

[dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }

[dev-dependencies]
foundation_nostd = { version = "0.0.4", features = ["std"] }
```

Create `.cargo/config.toml`:
```toml
# Development builds use std
[target.x86_64-unknown-linux-gnu]
rustflags = ["--cfg", "feature=\"std\""]

# Or use build profiles
[profile.dev]
# Will use dev-dependencies with std

[profile.release]
# Uses regular dependencies without std (unless specified)
```

**Build commands**:
```bash
# Development (with std)
cargo build

# Production (without std)
cargo build --release --no-default-features

# Production with std
cargo build --release --features std
```

---

## Template 8: Multi-Target with Target-Specific Features

**Use case**: Different platforms need different features

```toml
[package]
name = "my-multi-target-lib"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["lib", "staticlib", "cdylib"]

[features]
default = []
std = ["foundation_nostd/std"]

[dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }

# Linux/macOS/Windows (desktop) - use std
[target.'cfg(all(unix, not(target_arch = "wasm32")))'.dependencies]
foundation_nostd = { version = "0.0.4", features = ["std"] }

[target.'cfg(windows)'.dependencies]
foundation_nostd = { version = "0.0.4", features = ["std"] }

# Embedded targets - no std
[target.'cfg(target_os = "none")'.dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }

# WASM (browser) - no std
[target.'cfg(all(target_arch = "wasm32", not(target_os = "wasi")))'.dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }

# WASI - use std
[target.'cfg(target_os = "wasi")'.dependencies]
foundation_nostd = { version = "0.0.4", features = ["std"] }
```

**Build commands**:
```bash
# Linux (uses std automatically)
cargo build --release

# WASM browser (uses no_std automatically)
cargo build --release --target wasm32-unknown-unknown

# WASI (uses std automatically)
cargo build --release --target wasm32-wasi

# Embedded ARM (uses no_std automatically)
cargo build --release --target thumbv7em-none-eabihf
```

---

## Quick Selection Guide

| Your Project Type | Recommended Template |
|------------------|---------------------|
| Desktop/Server app | Template 1 |
| Embedded firmware | Template 2 |
| Cross-platform library | Template 3 |
| Web app (frontend + backend) | Template 4 |
| Monorepo/Workspace | Template 5 |
| Public library | Template 6 |
| Complex build configs | Template 7 or 8 |

---

## Testing Your Configuration

After setting up your `Cargo.toml`, verify it works:

```bash
# Test no_std build
cargo check --no-default-features

# Test std build
cargo check --features std

# Test WASM build
cargo check --target wasm32-unknown-unknown --no-default-features

# Run tests (uses dev-dependencies)
cargo test

# Check all combinations (requires cargo-hack)
cargo hack check --feature-powerset
```

---

## Common Patterns

### Pattern 1: Conditional Compilation in Code

```rust
use foundation_nostd::comp::Mutex;

#[cfg(feature = "std")]
pub fn create_mutex_std() -> Mutex<i32> {
    println!("Using std::sync::Mutex");
    Mutex::new(42)
}

#[cfg(not(feature = "std"))]
pub fn create_mutex_nostd() -> Mutex<i32> {
    // No println in no_std without alloc
    Mutex::new(42)
}
```

### Pattern 2: Optional Functionality

```rust
#[cfg(feature = "std")]
pub mod advanced_features {
    use foundation_nostd::comp::{Mutex, RwLock};

    pub fn heavy_computation() -> Mutex<Vec<i32>> {
        // This module only exists with std
        Mutex::new(vec![1, 2, 3])
    }
}

// Always available
pub mod core_features {
    use foundation_nostd::comp::Mutex;

    pub fn basic_counter() -> Mutex<i32> {
        Mutex::new(0)
    }
}
```

### Pattern 3: Re-export Based on Features

```rust
// lib.rs
#[cfg(feature = "std")]
pub use foundation_nostd::comp::{Mutex, RwLock, CondVar};

#[cfg(not(feature = "std"))]
pub use foundation_nostd::comp::{Mutex, RwLock};
// CondVar might not be suitable for your no_std use case
```
