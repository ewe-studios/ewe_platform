//! Cross-platform example demonstrating automatic std/no_std selection.
//!
//! This example shows how to write code once that automatically uses:
//! - `std::sync` types on native platforms
//! - `foundation_nostd` spin locks on WASM
//!
//! Build commands:
//! ```bash
//! # Native (automatically uses std)
//! cargo run --example cross_platform --release
//!
//! # WASM (automatically uses no_std)
//! cargo build --example cross_platform --target wasm32-unknown-unknown --release
//! ```

use foundation_nostd::comp::basic::{Mutex, Once, OnceLock, RwLock};

#[cfg(not(target_arch = "wasm32"))]
use std::thread;

// Global shared state using OnceLock
static GLOBAL_CONFIG: OnceLock<Mutex<Config>> = OnceLock::new();

// One-time initialization
static INIT: Once = Once::new();

#[derive(Debug, Clone)]
struct Config {
    name: String,
    value: i32,
}

impl Config {
    fn new(name: &str, value: i32) -> Self {
        Self {
            name: name.to_string(),
            value,
        }
    }
}

/// Shared counter using RwLock for efficient concurrent reads
struct SharedCounter {
    inner: RwLock<i32>,
}

impl SharedCounter {
    fn new(initial: i32) -> Self {
        Self {
            inner: RwLock::new(initial),
        }
    }

    fn increment(&self) {
        let mut guard = self.inner.write().unwrap();
        *guard += 1;
    }

    fn get(&self) -> i32 {
        let guard = self.inner.read().unwrap();
        *guard
    }
}

fn initialize_global() {
    INIT.call_once(|| {
        println!("🚀 Initializing global configuration...");
        let config = Config::new("MyApp", 100);
        GLOBAL_CONFIG.set(Mutex::new(config)).ok();
        println!("✓ Global configuration initialized");
    });
}

fn main() {
    println!("=== Cross-Platform Sync Demo ===\n");

    // Show which mode we're using
    #[cfg(feature = "std")]
    println!("🔧 Mode: Using std::sync (native platform)");

    #[cfg(not(feature = "std"))]
    println!("🔧 Mode: Using foundation_nostd spin locks (no_std)");

    #[cfg(target_arch = "wasm32")]
    println!("🌐 Target: WebAssembly");

    #[cfg(not(target_arch = "wasm32"))]
    println!("💻 Target: Native");

    println!();

    // Initialize global configuration
    initialize_global();
    initialize_global(); // Second call does nothing (Once guarantees)
    println!();

    // Access global config
    if let Some(config_mutex) = GLOBAL_CONFIG.get() {
        let mut config = config_mutex.lock().unwrap();
        println!(
            "📋 Global config: name={}, value={}",
            config.name, config.value
        );
        config.value += 50;
        println!("📋 Updated config value to: {}", config.value);
        drop(config);
    }
    println!();

    // Demonstrate RwLock with shared counter
    println!("🔢 Testing SharedCounter with RwLock:");
    let counter = SharedCounter::new(0);

    // Multiple readers can access simultaneously
    println!("   Initial value: {}", counter.get());

    // Increment multiple times
    for i in 1..=5 {
        counter.increment();
        println!("   After increment {}: {}", i, counter.get());
    }
    println!();

    // Demonstrate multi-threaded access (native only)
    #[cfg(not(target_arch = "wasm32"))]
    {
        println!("🧵 Testing multi-threaded access (native only):");
        use std::sync::Arc;

        let shared_counter = Arc::new(SharedCounter::new(0));
        let mut handles = vec![];

        // Spawn 5 threads that each increment 100 times
        for thread_id in 0..5 {
            let counter = Arc::clone(&shared_counter);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    counter.increment();
                }
                println!("   Thread {} completed", thread_id);
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        println!("   ✓ Final counter value: {}", shared_counter.get());
        println!("   Expected: 500, Got: {}", shared_counter.get());
        assert_eq!(shared_counter.get(), 500);
        println!();
    }

    // Demonstrate Mutex for exclusive access
    println!("🔒 Testing Mutex for exclusive access:");
    let data = Mutex::new(vec![1, 2, 3]);

    {
        let mut guard = data.lock().unwrap();
        guard.push(4);
        guard.push(5);
        println!("   Data after push: {:?}", *guard);
    } // Lock released here

    {
        let guard = data.lock().unwrap();
        println!("   Final data: {:?}", *guard);
    }
    println!();

    // Demonstrate nested locks (with care!)
    println!("🔐 Testing nested locks:");
    let outer = Mutex::new(42);
    let inner = Mutex::new("hello");

    {
        let outer_guard = outer.lock().unwrap();
        println!("   Outer lock acquired: {}", *outer_guard);

        {
            let inner_guard = inner.lock().unwrap();
            println!("   Inner lock acquired: {}", *inner_guard);
            println!("   Both locks held simultaneously");
        } // Inner lock released

        println!("   Inner lock released, outer still held");
    } // Outer lock released
    println!();

    // Performance characteristics info
    #[cfg(feature = "std")]
    println!("ℹ️  Performance: Using OS-level blocking primitives (optimal for contention)");

    #[cfg(not(feature = "std"))]
    println!("ℹ️  Performance: Using spin locks (optimal for short critical sections)");

    #[cfg(target_arch = "wasm32")]
    println!("ℹ️  WASM Note: Single-threaded environment, locks are essentially no-ops");

    println!("\n=== Demo Complete ===");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_counter() {
        let counter = SharedCounter::new(10);
        assert_eq!(counter.get(), 10);

        counter.increment();
        assert_eq!(counter.get(), 11);

        counter.increment();
        counter.increment();
        assert_eq!(counter.get(), 13);
    }

    #[test]
    fn test_once_initialization() {
        static TEST_ONCE: Once = Once::new();
        let mut counter = 0;

        TEST_ONCE.call_once(|| {
            counter += 1;
        });

        TEST_ONCE.call_once(|| {
            counter += 1;
        });

        assert_eq!(counter, 1);
    }

    #[test]
    fn test_mutex_basic() {
        let mutex = Mutex::new(vec![1, 2, 3]);

        {
            let mut guard = mutex.lock().unwrap();
            guard.push(4);
        }

        let guard = mutex.lock().unwrap();
        assert_eq!(*guard, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_rwlock_basic() {
        let rwlock = RwLock::new(42);

        // Test read
        {
            let read = rwlock.read().unwrap();
            assert_eq!(*read, 42);
        }

        // Test write
        {
            let mut write = rwlock.write().unwrap();
            *write = 100;
        }

        // Verify
        let read = rwlock.read().unwrap();
        assert_eq!(*read, 100);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;

        let counter = Arc::new(SharedCounter::new(0));
        let mut handles = vec![];

        for _ in 0..10 {
            let counter = Arc::clone(&counter);
            let handle = thread::spawn(move || {
                for _ in 0..10 {
                    counter.increment();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(counter.get(), 100);
    }
}
