# Quick Start Guide: `foundation_nostd::comp`

Get started with the compatibility module in 5 minutes!

## 1. Add Dependency

Add to your `Cargo.toml`:

```toml
[dependencies]
foundation_nostd = "0.0.4"
```

## 2. Choose Your Configuration

### Option A: Auto-detect based on platform (Recommended)

```toml
[dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }

# Automatically use std on native platforms
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
foundation_nostd = { version = "0.0.4", features = ["std"] }
```

### Option B: Always use std

```toml
[dependencies]
foundation_nostd = { version = "0.0.4", features = ["std"] }
```

### Option C: Always use no_std

```toml
[dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }
```

## 3. Use in Your Code

```rust
use foundation_nostd::comp::{Mutex, RwLock};

fn main() {
    // Works with both std and no_std!
    let mutex = Mutex::new(42);
    let mut guard = mutex.lock().unwrap();
    *guard += 1;
    println!("Value: {}", *guard);
}
```

## 4. Build and Run

```bash
# Build with auto-detection
cargo build

# Explicitly build with std
cargo build --features std

# Explicitly build without std
cargo build --no-default-features

# Build for WASM
cargo build --target wasm32-unknown-unknown
```

## Common Use Cases

### Use Case 1: Simple Desktop Application

**Goal**: Use standard library for best performance

```toml
[dependencies]
foundation_nostd = { version = "0.0.4", features = ["std"] }
```

```rust
use foundation_nostd::comp::Mutex;

fn main() {
    let data = Mutex::new(vec![1, 2, 3]);
    let mut guard = data.lock().unwrap();
    guard.push(4);
    println!("{:?}", *guard);
}
```

### Use Case 2: Embedded System

**Goal**: No standard library available

```toml
[dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }
```

```rust
#![no_std]

use foundation_nostd::comp::Mutex;

static COUNTER: Mutex<u32> = Mutex::new(0);

#[no_mangle]
pub extern "C" fn increment_counter() {
    let mut guard = COUNTER.lock().unwrap();
    *guard += 1;
}
```

### Use Case 3: Cross-Platform Library

**Goal**: Support both native and WASM

```toml
[lib]
crate-type = ["lib", "cdylib"]

[features]
default = []
std = ["foundation_nostd/std"]

[dependencies]
foundation_nostd = { version = "0.0.4", default-features = false }

# Auto-enable std on native
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
foundation_nostd = { version = "0.0.4", features = ["std"] }
```

```rust
use foundation_nostd::comp::{Mutex, RwLock};

pub struct SharedData {
    counter: Mutex<i32>,
    values: RwLock<Vec<i32>>,
}

impl SharedData {
    pub fn new() -> Self {
        Self {
            counter: Mutex::new(0),
            values: RwLock::new(Vec::new()),
        }
    }

    pub fn increment(&self) {
        let mut guard = self.counter.lock().unwrap();
        *guard += 1;
    }

    pub fn add_value(&self, val: i32) {
        let mut guard = self.values.write().unwrap();
        guard.push(val);
    }
}
```

## Available Types

### Core Synchronization
- `Mutex<T>` - Mutual exclusion lock
- `RwLock<T>` - Read-write lock
- `CondVar` - Condition variable

### One-Time Initialization
- `Once` - One-time initialization
- `OnceLock<T>` - Lazy initialized cell

### Thread Coordination
- `Barrier` - Thread barrier

### Error Types
- `PoisonError<T>` - Lock poisoning error
- `TryLockError<T>` - Try-lock failure

## Testing

Test your code in both modes:

```bash
# Test with no_std
cargo test --no-default-features

# Test with std
cargo test --features std

# Test all examples
cargo test --examples
```

## Real-World Example

Here's a complete example of a counter that works everywhere:

```rust
use foundation_nostd::comp::{Mutex, RwLock};

pub struct ThreadSafeCounter {
    // Use Mutex for exclusive access
    count: Mutex<u64>,
    // Use RwLock for read-heavy data
    history: RwLock<Vec<u64>>,
}

impl ThreadSafeCounter {
    pub fn new() -> Self {
        Self {
            count: Mutex::new(0),
            history: RwLock::new(Vec::new()),
        }
    }

    pub fn increment(&self) -> u64 {
        let mut count = self.count.lock().unwrap();
        *count += 1;
        let new_value = *count;

        // Record in history
        let mut history = self.history.write().unwrap();
        history.push(new_value);

        new_value
    }

    pub fn get(&self) -> u64 {
        let count = self.count.lock().unwrap();
        *count
    }

    pub fn history_len(&self) -> usize {
        let history = self.history.read().unwrap();
        history.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter() {
        let counter = ThreadSafeCounter::new();
        assert_eq!(counter.get(), 0);

        counter.increment();
        assert_eq!(counter.get(), 1);
        assert_eq!(counter.history_len(), 1);

        counter.increment();
        counter.increment();
        assert_eq!(counter.get(), 3);
        assert_eq!(counter.history_len(), 3);
    }
}
```

## Next Steps

1. Check out [README.md](README.md) for complete documentation
2. Browse [CONFIGURATION_TEMPLATES.md](CONFIGURATION_TEMPLATES.md) for advanced setups
3. Run examples: `cargo run --example comp_usage`
4. Read the [API documentation](https://docs.rs/foundation_nostd)

## Troubleshooting

### Error: "no_std is not compatible with std"

**Problem**: You're trying to use std features in a no_std build.

**Solution**: Either enable the `std` feature or avoid using std-only APIs:
```bash
cargo build --features std
```

### Error: "cannot find type `Mutex` in crate `comp`"

**Problem**: The `comp` module isn't imported.

**Solution**: Add the import:
```rust
use foundation_nostd::comp::Mutex;
```

### Build works on native but fails on WASM

**Problem**: You might be using std-only features in your code.

**Solution**: Use conditional compilation:
```rust
#[cfg(feature = "std")]
use std::time::Instant;

#[cfg(not(feature = "std"))]
use core::time::Duration;
```

### Tests fail with "poisoned lock"

**Problem**: A panic occurred while holding a lock.

**Solution**: This is normal behavior. You can recover:
```rust
let result = mutex.lock();
match result {
    Ok(guard) => { /* use guard */ },
    Err(poisoned) => {
        // Recover the guard
        let guard = poisoned.into_inner();
        // Use it anyway
    }
}
```

## Performance Tips

### When to use `std` feature:
- ✅ Long-held locks
- ✅ High contention scenarios
- ✅ Desktop/server applications
- ✅ When you need proper thread blocking

### When to use `no_std` (spin locks):
- ✅ Short critical sections
- ✅ Low contention
- ✅ Embedded systems
- ✅ WASM environments
- ✅ When std isn't available

### Best Practices:
- Keep critical sections short
- Avoid nested locks when possible
- Use `RwLock` for read-heavy scenarios
- Profile your specific use case

## Getting Help

- 📖 Full documentation: [README.md](README.md)
- 🔧 Configuration templates: [CONFIGURATION_TEMPLATES.md](CONFIGURATION_TEMPLATES.md)
- 💡 Examples: `examples/` directory
- 🐛 Issues: GitHub repository

Happy coding! 🚀
