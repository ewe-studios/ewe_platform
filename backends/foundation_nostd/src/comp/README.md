# Compatibility Module (`comp`)

The `comp` module provides a compatibility layer for synchronization primitives that automatically selects between `std` and `no_std` implementations based on the `std` feature flag.

## Overview

This module allows users to write code once that works seamlessly in both standard and `no_std` environments. When the `std` feature is enabled, the module uses the standard library's optimized primitives. When disabled (default), it uses `foundation_nostd`'s spin-lock based implementations.

## Usage

### Basic Configuration

#### For `std` environments (Desktop, Server):
```toml
[dependencies]
foundation_nostd = { version = "0.0.4", features = ["std"] }
```

#### For `no_std` environments (Embedded, WASM):
```toml
[dependencies]
foundation_nostd = "0.0.4"
# or explicitly:
foundation_nostd = { version = "0.0.4", default-features = false }
```

### Advanced Configuration Examples

#### Environment-Specific Feature Selection

##### 1. **Automatic std based on target platform**

Enable `std` for native targets, disable for WASM:

```toml
[dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
foundation_nostd = { version = "0.0.4", features = ["std"] }
```

##### 2. **Enable std only for specific WASM variants**

Use `std` for native and WASI, but not for browser WASM:

```toml
[dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }

# Enable std for native targets
[target.'cfg(not(target_family = "wasm"))'.dependencies]
foundation_nostd = { version = "0.0.4", features = ["std"] }

# Enable std for WASI (WASM with std support)
[target.'cfg(target_os = "wasi")'.dependencies]
foundation_nostd = { version = "0.0.4", features = ["std"] }
```

##### 3. **Workspace-level feature propagation**

If you're in a workspace, propagate the feature from your main crate:

```toml
# In your library crate's Cargo.toml
[features]
default = []
std = ["foundation_nostd/std"]

[dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }
```

Then users can enable it:
```toml
# User's Cargo.toml
[dependencies]
your_crate = { version = "1.0", features = ["std"] }
```

##### 4. **Optional std support with feature gate**

Create an optional dependency that uses `std` when available:

```toml
[features]
default = ["std"]
std = ["foundation_nostd/std"]

[dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }
```

##### 5. **Multi-platform library with conditional std**

For a library that supports embedded, WASM, and native:

```toml
[features]
# Default to no_std for maximum compatibility
default = []

# Enable std for native platforms
std = ["foundation_nostd/std"]

[dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }

# Automatically enable std for known platforms with std support
[target.'cfg(all(not(target_arch = "wasm32"), not(target_os = "none")))'.dependencies]
foundation_nostd = { version = "0.0.4", default-features = false, features = ["std"] }
```

##### 6. **Dev/test with std, release without**

Use `std` in tests and dev builds, but compile without for release:

```toml
[dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }

[dev-dependencies]
foundation_nostd = { version = "0.0.4", features = ["std"] }

# For tests, override to use std
[profile.test]
# Tests will use the dev-dependencies version with std
```

##### 7. **Environment variable controlled builds**

Use build scripts to conditionally enable features:

```toml
# Cargo.toml
[features]
std = ["foundation_nostd/std"]

[dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }

[build-dependencies]
# If you need build.rs for custom logic
```

```rust
// build.rs
fn main() {
    // Enable std feature when ENABLE_STD env var is set
    if std::env::var("ENABLE_STD").is_ok() {
        println!("cargo:rustc-cfg=feature=\"std\"");
    }

    // Or based on target
    let target = std::env::var("TARGET").unwrap();
    if !target.contains("wasm32") && !target.contains("none") {
        println!("cargo:rustc-cfg=feature=\"std\"");
    }
}
```

Then build with:
```bash
# Force std
ENABLE_STD=1 cargo build

# Or use cargo features directly
cargo build --features std
cargo build --no-default-features  # Explicitly no std
```

### Real-World Configuration Examples

#### Example 1: Cross-Platform Application

```toml
[package]
name = "my_app"

[features]
default = []
std = ["foundation_nostd/std"]

[dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }

# Desktop/Server builds
[target.'cfg(all(not(target_arch = "wasm32"), not(target_vendor = "unknown")))'.dependencies]
foundation_nostd = { version = "0.0.4", features = ["std"] }
```

#### Example 2: Embedded System Library

```toml
[package]
name = "embedded_lib"

[features]
# No std by default for embedded
default = []

# Optional std support for testing on desktop
std = ["foundation_nostd/std"]

[dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }

[dev-dependencies]
# Tests run with std on the host machine
foundation_nostd = { version = "0.0.4", features = ["std"] }
```

#### Example 3: WASM + Native Library

```toml
[package]
name = "wasm_native_lib"

[lib]
crate-type = ["cdylib", "rlib"]

[features]
default = []
std = ["foundation_nostd/std"]

[dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }

# Enable std for non-WASM targets
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
foundation_nostd = { version = "0.0.4", features = ["std"] }

# For WASM, use no_std (spinlocks)
[target.'cfg(target_arch = "wasm32")'.dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }
```

Build commands:
```bash
# Native build (uses std)
cargo build --release

# WASM build (uses no_std)
cargo build --release --target wasm32-unknown-unknown

# WASI build (can use std if enabled)
cargo build --release --target wasm32-wasi --features std
```

### Quick Reference: When to Use Which Configuration

| Scenario | Configuration |
|----------|--------------|
| **Standard Rust application** | `features = ["std"]` |
| **Embedded (bare metal)** | `default-features = false` |
| **WASM (browser)** | `default-features = false` |
| **WASM (WASI/Node)** | `features = ["std"]` (if target supports) |
| **Cross-platform library** | Conditional features based on target |
| **Testing no_std code** | `default-features = false` in deps, `features = ["std"]` in dev-deps |

### Example Code

```rust
use foundation_nostd::comp::{Mutex, RwLock, CondVar, CondVarMutex};

fn main() {
    // Mutex - works with both std and no_std
    let mutex = Mutex::new(42);
    let guard = mutex.lock().unwrap();
    println!("Value: {}", *guard);
    drop(guard);

    // RwLock - works with both std and no_std
    let rwlock = RwLock::new(vec![1, 2, 3]);

    // Multiple readers
    let r1 = rwlock.read().unwrap();
    let r2 = rwlock.read().unwrap();
    println!("Readers: {:?}, {:?}", *r1, *r2);
    drop((r1, r2));

    // Single writer
    let mut w = rwlock.write().unwrap();
    w.push(4);
    drop(w);

    // CondVar - works with both std and no_std
    let mutex = CondVarMutex::new(false);
    let condvar = CondVar::new();

    let mut ready = mutex.lock().unwrap();
    while !*ready {
        ready = condvar.wait(ready).unwrap();
    }
}
```

## Supported Types

### Mutex Types
- `Mutex<T>` - Standard mutex with poisoning support
- `MutexGuard<'a, T>` - RAII guard for mutex

### RwLock Types
- `RwLock<T>` - Read-write lock
- `RwLockReadGuard<'a, T>` - RAII guard for read access
- `RwLockWriteGuard<'a, T>` - RAII guard for write access

### CondVar Types
- `CondVar` - Condition variable
- `CondVarMutex<T>` - Mutex designed for use with condition variables
- `CondVarMutexGuard<'a, T>` - Guard for CondVarMutex
- `WaitTimeoutResult` - Result of timed wait operations

### Synchronization Types
- `Barrier` - Barrier for thread synchronization
- `BarrierWaitResult` - Result of barrier wait operation
- `Once` - One-time initialization
- `OnceState` - State of Once initialization
- `OnceLock<T>` - Thread-safe cell for one-time initialization

### Error Types
- `PoisonError<T>` - Error indicating a poisoned lock
- `TryLockError<T>` - Error from try_lock operations
- `LockResult<T>` - Result type with PoisonError
- `TryLockResult<T>` - Result type with TryLockError

### Foundation-Specific Types (no_std only)

The following types are only available when the `std` feature is **not** enabled, as they are specific to the `foundation_nostd` crate:

- `SpinMutex<T>`, `RawSpinMutex<T>` - Spin-lock based mutexes
- `SpinRwLock<T>`, `RawSpinRwLock<T>`, `ReaderSpinRwLock<T>` - Spin-lock based RwLocks
- `RwLockCondVar` - Condition variable for RwLocks
- `CondVarNonPoisoning` - Non-poisoning condition variable
- `AtomicCell<T>`, `AtomicFlag`, `AtomicLazy<T>`, `AtomicOption<T>` - Atomic types
- `SpinWait` - Spin-wait helper

## Feature Comparison

| Feature | With `std` | Without `std` |
|---------|-----------|---------------|
| **Implementation** | Uses `std::sync` types | Uses spin-lock implementations |
| **Performance** | OS-level primitives (best) | Busy-waiting (good for short locks) |
| **Blocking** | Proper thread blocking | Spin-waiting |
| **Memory** | Slightly larger | Minimal memory footprint |
| **Poisoning** | Full poisoning support | Full poisoning support |
| **API Compatibility** | 100% `std::sync` compatible | Drop-in replacement API |

## When to Use Each Mode

### Use `std` mode when:
- You have a standard library available
- You need optimal performance for heavily contended locks
- You want proper OS-level thread blocking
- You're building for desktop/server environments

### Use `no_std` mode when:
- You're building for embedded systems
- You're targeting WebAssembly
- You need `no_std` compatibility
- Lock contention is minimal or locks are held briefly

## Migration from `std::sync`

Migrating existing code is straightforward:

```rust
// Before:
use std::sync::{Mutex, RwLock, Condvar};

// After:
use foundation_nostd::comp::{Mutex, RwLock, CondVar};
```

The API is identical, so your existing code should work without changes!

## Examples

See `examples/comp_usage.rs` for a comprehensive example demonstrating all features:

```bash
# Run with std
cargo run --example comp_usage --features std

# Run without std
cargo run --example comp_usage --no-default-features
```

## Relationship to `foundation_core::compati`

The `foundation_core` crate has a similar `compati` module that switches based on **target architecture** (WASM vs non-WASM), while this `comp` module switches based on the **`std` feature flag**. This gives you more explicit control over which implementation to use.

If you're using `foundation_core`, you can combine both:
- Use `foundation_core::compati` for platform-specific selection (WASM vs native)
- Use `foundation_nostd::comp` for environment-specific selection (std vs no_std)

## Implementation Details

### With `std` feature enabled:
All types are re-exported directly from `std::sync`, providing zero-cost abstraction.

### Without `std` feature (default):
- `Mutex` → `foundation_nostd::primitives::Mutex` (platform-appropriate)
- `RwLock` → `foundation_nostd::primitives::RwLock` (platform-appropriate)
- `CondVar` → `foundation_nostd::primitives::CondVar`
- All guard types use the corresponding primitive implementations

The platform-appropriate types automatically select:
- On WASM without atomics: No-op implementations
- On all other platforms: Spin-lock implementations

## Thread Safety

Both `std` and `no_std` implementations are fully thread-safe and implement the appropriate `Send` and `Sync` traits.

## Testing

The module includes comprehensive tests that run in both modes:

```bash
# Test without std
cargo test --no-default-features comp

# Test with std
cargo test --features std comp
```
