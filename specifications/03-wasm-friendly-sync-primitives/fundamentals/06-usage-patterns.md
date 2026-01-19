# Usage Patterns and Best Practices

## Choosing the Right Primitive

### Decision Matrix

| Need | Primitive | When to Use |
|------|-----------|-------------|
| Exclusive access to data | `SpinMutex<T>` | General purpose, need panic safety |
| Exclusive access (embedded) | `RawSpinMutex<T>` | panic=abort, simpler API |
| Multiple readers, one writer | `SpinRwLock<T>` | Read-heavy workloads |
| One-time initialization | `Once` + `OnceLock<T>` | Lazy statics, config |
| Small atomic values | `AtomicCell<T>` | Counters, flags, small structs |
| Atomic optional pointer | `AtomicOption<T>` | Lock-free data structures |
| Simple boolean flag | `AtomicFlag` | Simpler than AtomicBool |
| Wait for N threads | `SpinBarrier` | Parallel computation phases |

---

## Pattern 1: Protecting Shared State

### Basic Mutex Usage

```rust
use foundation_nostd::primitives::SpinMutex;

struct SharedState {
    counter: u32,
    data: Vec<u8>,
}

// Create shared state
static STATE: SpinMutex<SharedState> = SpinMutex::new(SharedState {
    counter: 0,
    data: Vec::new(),
});

fn update_state() {
    let mut guard = STATE.lock().unwrap();
    guard.counter += 1;
    guard.data.push(guard.counter as u8);
} // Lock released here

fn read_counter() -> u32 {
    STATE.lock().unwrap().counter
}
```

### Minimizing Lock Duration

```rust
// BAD: Holding lock during I/O
fn bad_save() {
    let guard = STATE.lock().unwrap();
    std::fs::write("data.bin", &guard.data)?; // Long I/O while holding lock!
}

// GOOD: Clone and release quickly
fn good_save() {
    let data = {
        let guard = STATE.lock().unwrap();
        guard.data.clone()
    }; // Lock released
    std::fs::write("data.bin", &data)?; // I/O outside lock
}
```

### Using try_lock for Non-Blocking

```rust
fn maybe_update() -> bool {
    match STATE.try_lock() {
        Ok(mut guard) => {
            guard.counter += 1;
            true
        }
        Err(_) => false, // Someone else has the lock
    }
}

// With spin limit
fn update_with_timeout() -> bool {
    match STATE.try_lock_with_spin_limit(10_000) {
        Ok(mut guard) => {
            guard.counter += 1;
            true
        }
        Err(_) => {
            log::warn!("Couldn't acquire lock after 10k spins");
            false
        }
    }
}
```

---

## Pattern 2: Read-Heavy Workloads

### RwLock for Concurrent Reads

```rust
use foundation_nostd::primitives::SpinRwLock;

struct Config {
    max_connections: u32,
    timeout_ms: u64,
    features: Vec<String>,
}

static CONFIG: SpinRwLock<Config> = SpinRwLock::new(Config {
    max_connections: 100,
    timeout_ms: 5000,
    features: Vec::new(),
});

// Many threads can read simultaneously
fn get_timeout() -> u64 {
    CONFIG.read().unwrap().timeout_ms
}

fn check_feature(name: &str) -> bool {
    CONFIG.read().unwrap().features.contains(&name.to_string())
}

// Only one thread can write
fn update_timeout(new_timeout: u64) {
    CONFIG.write().unwrap().timeout_ms = new_timeout;
}
```

### When to Use RwLock vs Mutex

| Scenario | Better Choice |
|----------|---------------|
| Read:Write ratio > 10:1 | RwLock |
| Read:Write ratio < 10:1 | Mutex |
| Short critical sections | Mutex (simpler) |
| Write-heavy | Mutex |
| Read-heavy with rare updates | RwLock |

---

## Pattern 3: Lazy Initialization

### Once for Side Effects

```rust
use foundation_nostd::primitives::Once;

static INIT: Once = Once::new();

fn ensure_initialized() {
    INIT.call_once(|| {
        // This runs exactly once, even with concurrent calls
        setup_logging();
        load_config();
        connect_to_database();
    });
}
```

### OnceLock for Values

```rust
use foundation_nostd::primitives::OnceLock;

static CONFIG: OnceLock<Config> = OnceLock::new();

fn get_config() -> &'static Config {
    CONFIG.get_or_init(|| {
        Config::load_from_file("config.toml")
    })
}

// Or with fallible initialization
fn get_config_result() -> Result<&'static Config, Error> {
    CONFIG.get_or_try_init(|| {
        Config::load_from_file("config.toml")
    })
}
```

### Lazy Static Alternative

```rust
use foundation_nostd::primitives::AtomicLazy;

// Initialize on first access
static CACHE: AtomicLazy<HashMap<String, String>> = AtomicLazy::new(|| {
    let mut map = HashMap::new();
    map.insert("version".into(), "1.0".into());
    map
});

fn get_cached(key: &str) -> Option<&'static String> {
    CACHE.get(key)
}
```

---

## Pattern 4: Lock-Free Counters

### Using AtomicCell

```rust
use foundation_nostd::primitives::AtomicCell;

static REQUESTS: AtomicCell<u64> = AtomicCell::new(0);
static ERRORS: AtomicCell<u64> = AtomicCell::new(0);

fn handle_request() {
    let count = REQUESTS.load();
    REQUESTS.store(count + 1);

    if let Err(_) = process() {
        let errors = ERRORS.load();
        ERRORS.store(errors + 1);
    }
}

fn get_stats() -> (u64, u64) {
    (REQUESTS.load(), ERRORS.load())
}
```

### Using AtomicFlag

```rust
use foundation_nostd::primitives::AtomicFlag;

static SHUTDOWN: AtomicFlag = AtomicFlag::new();

fn request_shutdown() {
    if !SHUTDOWN.test_and_set() {
        // First to request shutdown
        log::info!("Shutdown requested");
        notify_all_workers();
    }
}

fn should_continue() -> bool {
    !SHUTDOWN.is_set()
}

fn worker_loop() {
    while should_continue() {
        process_next_task();
    }
}
```

---

## Pattern 5: Safe Initialization

### Double-Checked Locking

```rust
use foundation_nostd::primitives::{SpinMutex, OnceLock};

struct ExpensiveResource {
    // ...
}

struct ResourceHolder {
    init_lock: SpinMutex<()>,
    resource: OnceLock<ExpensiveResource>,
}

impl ResourceHolder {
    pub fn get(&self) -> &ExpensiveResource {
        // Fast path: already initialized
        if let Some(resource) = self.resource.get() {
            return resource;
        }

        // Slow path: initialize with lock
        let _guard = self.init_lock.lock().unwrap();

        // Double-check after acquiring lock
        self.resource.get_or_init(|| {
            ExpensiveResource::new()
        })
    }
}
```

---

## Pattern 6: Poisoning Recovery

### Handling Poisoned Locks

```rust
use foundation_nostd::primitives::SpinMutex;

let counter = SpinMutex::new(0u32);

// Simulate panic in another thread
std::panic::catch_unwind(|| {
    let mut guard = counter.lock().unwrap();
    *guard = 42;
    panic!("Oops!");
});

// Handle poisoned lock
let value = match counter.lock() {
    Ok(guard) => *guard,
    Err(poisoned) => {
        // Option 1: Propagate
        // panic!("Lock poisoned");

        // Option 2: Recover
        log::warn!("Recovering from poisoned lock");
        let guard = poisoned.into_inner();
        *guard // Use the value anyway
    }
};
```

### Clear Poison Pattern

```rust
fn reset_and_continue(mutex: &SpinMutex<State>) -> State {
    match mutex.lock() {
        Ok(guard) => (*guard).clone(),
        Err(poisoned) => {
            let mut guard = poisoned.into_inner();

            // Reset to known-good state
            *guard = State::default();

            (*guard).clone()
        }
    }
}
```

---

## Pattern 7: Barrier Synchronization

### Parallel Computation Phases

```rust
use foundation_nostd::primitives::SpinBarrier;
use std::thread;

fn parallel_matrix_multiply(a: &Matrix, b: &Matrix) -> Matrix {
    let barrier = SpinBarrier::new(4); // 4 worker threads
    let result = SpinMutex::new(Matrix::zeros());

    thread::scope(|s| {
        for i in 0..4 {
            let barrier = &barrier;
            let result = &result;

            s.spawn(move || {
                // Phase 1: Compute partial result
                let partial = compute_rows(a, b, i);

                // Wait for all threads to finish phase 1
                barrier.wait();

                // Phase 2: Merge results
                result.lock().unwrap().merge(partial);

                // Wait for all merges
                barrier.wait();
            });
        }
    });

    result.into_inner().unwrap()
}
```

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Lock Ordering Violations

```rust
// DEADLOCK POTENTIAL!
fn transfer_bad(from: &SpinMutex<Account>, to: &SpinMutex<Account>) {
    let mut from_guard = from.lock().unwrap();
    let mut to_guard = to.lock().unwrap(); // Deadlock if another thread does opposite order!
    // ...
}

// FIXED: Always lock in consistent order
fn transfer_good(from: &SpinMutex<Account>, to: &SpinMutex<Account>) {
    // Use pointer addresses for consistent ordering
    let (first, second) = if std::ptr::from_ref(from) < std::ptr::from_ref(to) {
        (from, to)
    } else {
        (to, from)
    };

    let mut first_guard = first.lock().unwrap();
    let mut second_guard = second.lock().unwrap();
    // ...
}
```

### Anti-Pattern 2: Holding Locks Across Await Points

```rust
// BAD: Lock held across await (in async contexts)
async fn bad_async() {
    let guard = MUTEX.lock().unwrap();
    some_async_operation().await; // Lock held during await!
    drop(guard);
}

// GOOD: Release before await
async fn good_async() {
    {
        let guard = MUTEX.lock().unwrap();
        // Quick synchronous work
    } // Lock released
    some_async_operation().await;
}
```

### Anti-Pattern 3: Recursive Locking

```rust
// DEADLOCK!
fn recursive_deadlock(mutex: &SpinMutex<u32>) {
    let guard = mutex.lock().unwrap();
    // ... some logic ...
    let guard2 = mutex.lock().unwrap(); // Same mutex - deadlock!
}

// FIXED: Restructure to avoid recursion
fn fixed(mutex: &SpinMutex<u32>) {
    let value = {
        let guard = mutex.lock().unwrap();
        *guard
    };
    // Work with value outside lock
    process(value);
}
```

### Anti-Pattern 4: Unbounded Spinning

```rust
// BAD: Could spin forever
let guard = mutex.lock().unwrap();

// GOOD: Use bounded spinning in production
match mutex.try_lock_with_spin_limit(10_000) {
    Ok(guard) => { /* use guard */ }
    Err(_) => {
        log::error!("Lock acquisition timeout");
        return Err(Error::Timeout);
    }
}
```

---

## Performance Tips

### 1. Prefer Atomics for Simple Values

```rust
// Overkill for a counter
let counter: SpinMutex<u64> = SpinMutex::new(0);

// Better
let counter: AtomicCell<u64> = AtomicCell::new(0);
```

### 2. Use Raw Variants When Appropriate

```rust
// If panics abort (embedded, panic=abort):
let mutex: RawSpinMutex<Data> = RawSpinMutex::new(data);
let guard = mutex.lock(); // No Result, simpler
```

### 3. Batch Updates

```rust
// BAD: Multiple lock acquisitions
for item in items {
    mutex.lock().unwrap().push(item);
}

// GOOD: Single lock acquisition
{
    let mut guard = mutex.lock().unwrap();
    for item in items {
        guard.push(item);
    }
}
```

### 4. Read-Only Access with RwLock

```rust
// If you only need to read
let value = rwlock.read().unwrap().get_value();

// Don't do this for read-only access
let value = rwlock.write().unwrap().get_value(); // Blocks all readers!
```

---

## Testing Concurrent Code

### Basic Thread Safety Test

```rust
#[test]
fn test_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let counter = Arc::new(SpinMutex::new(0u32));
    let mut handles = vec![];

    for _ in 0..10 {
        let counter = Arc::clone(&counter);
        handles.push(thread::spawn(move || {
            for _ in 0..1000 {
                *counter.lock().unwrap() += 1;
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(*counter.lock().unwrap(), 10_000);
}
```

### Testing Poisoning

```rust
#[test]
fn test_poisoning() {
    let mutex = SpinMutex::new(42);

    let _ = std::panic::catch_unwind(|| {
        let _guard = mutex.lock().unwrap();
        panic!("test panic");
    });

    assert!(mutex.is_poisoned());

    match mutex.lock() {
        Ok(_) => panic!("Should be poisoned"),
        Err(e) => {
            assert_eq!(*e.into_inner(), 42);
        }
    }
}
```

---

## Summary

| Pattern | Use Case | Key Points |
|---------|----------|------------|
| **Basic Mutex** | Shared mutable state | Minimize lock duration |
| **RwLock** | Read-heavy access | Use when reads >> writes |
| **Once/OnceLock** | Lazy init | Thread-safe, exactly once |
| **AtomicCell** | Small values | Lock-free, fast |
| **AtomicFlag** | Boolean flags | Simpler than AtomicBool |
| **Barrier** | Phase sync | Wait for N threads |
| **try_lock** | Non-blocking | Handle contention gracefully |

**Golden Rules:**
1. Hold locks for minimal time
2. Never hold locks across await points
3. Use consistent lock ordering to prevent deadlocks
4. Prefer atomics for simple values
5. Use bounded spinning in production
6. Test concurrent code thoroughly

---

*Next: [07-implementation-guide.md](./07-implementation-guide.md) - Library internals and design*
