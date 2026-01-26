//! Example demonstrating the usage of the `comp` module.
//!
//! This example shows how to use the compatibility layer to write code
//! that works with both `std` and `no_std` environments.
//!
//! Run with std:
//! ```bash
//! cargo run --example comp_usage --features std
//! ```
//!
//! Run without std (will use spin locks):
//! ```bash
//! cargo run --example comp_usage --no-default-features
//! ```

use foundation_nostd::comp::basic::{Barrier, Mutex, Once, OnceLock, RwLock};
use foundation_nostd::comp::condvar_comp::{CondVar, CondVarMutex};
use std::time::Duration;

fn main() {
    println!("=== Foundation NoStd Comp Module Demo ===\n");

    // Mutex example
    println!("1. Mutex Example:");
    let mutex = Mutex::new(42);
    {
        let guard = mutex.lock().unwrap();
        println!("   Locked value: {}", *guard);
    }
    println!("   ✓ Mutex unlocked\n");

    // RwLock example
    println!("2. RwLock Example:");
    let rwlock = RwLock::new(vec![1, 2, 3]);
    {
        let read_guard = rwlock.read().unwrap();
        println!("   Read: {:?}", *read_guard);
    }
    {
        let mut write_guard = rwlock.write().unwrap();
        write_guard.push(4);
        println!("   After write: {:?}", *write_guard);
    }
    println!("   ✓ RwLock operations complete\n");

    // CondVar example
    println!("3. CondVar Example:");
    let mutex = CondVarMutex::new(false);
    let condvar = CondVar::new();
    {
        let guard = mutex.lock().unwrap();
        let result = condvar.wait_timeout(guard, Duration::from_millis(10));
        match result {
            Ok((_guard, timeout_result)) => {
                if timeout_result.timed_out() {
                    println!("   ✓ Wait timed out as expected");
                }
            }
            Err(_) => println!("   Mutex was poisoned"),
        }
    }
    println!();

    // Barrier example
    println!("4. Barrier Example:");
    let barrier = Barrier::new(1);
    let result = barrier.wait();
    println!("   Barrier wait result - is_leader: {}", result.is_leader());
    println!("   ✓ Barrier complete\n");

    // Once example
    println!("5. Once Example:");
    static INIT: Once = Once::new();
    let mut counter = 0;
    INIT.call_once(|| {
        counter += 1;
        println!("   Initialization called (counter = {})", counter);
    });
    INIT.call_once(|| {
        counter += 1;
        println!("   This should not print (counter would be {})", counter);
    });
    println!("   ✓ Once initialization complete\n");

    // OnceLock example
    println!("6. OnceLock Example:");
    let lock: OnceLock<String> = OnceLock::new();
    println!("   Initial value: {:?}", lock.get());
    lock.set("Hello, World!".to_string()).ok();
    println!("   After set: {:?}", lock.get());
    let result = lock.set("This won't work".to_string());
    println!("   Second set failed: {}", result.is_err());
    println!("   ✓ OnceLock operations complete\n");

    #[cfg(feature = "std")]
    println!("✓ Running with std feature enabled (using std::sync types)");

    #[cfg(not(feature = "std"))]
    println!("✓ Running in no_std mode (using foundation_nostd spin locks)");

    println!("\n=== Demo Complete ===");
}
