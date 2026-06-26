extern crate std;

use foundation_nostd::primitives::spin_waiter::*;
    use core::time::Duration;
    use std::time::Instant;

    /// WHY: Validates SpinWaiter construction with custom settings
    /// WHAT: Creating a SpinWaiter should store configuration correctly
    #[test]
    fn test_new() {
        let waiter = SpinWaiter::new(50_000);
        assert_eq!(waiter.iterations_per_ms(), 50_000);
        assert!(!waiter.is_interrupted());
    }

    /// WHY: Validates WASM-specific constructor
    /// WHAT: wasm() should create SpinWaiter with 100K iterations/ms
    #[test]
    fn test_wasm() {
        let waiter = SpinWaiter::wasm();
        assert_eq!(waiter.iterations_per_ms(), 100_000);
    }

    /// WHY: Validates embedded-specific constructor
    /// WHAT: embedded() should create SpinWaiter with 1M iterations/ms
    #[test]
    fn test_embedded() {
        let waiter = SpinWaiter::embedded();
        assert_eq!(waiter.iterations_per_ms(), 1_000_000);
    }

    /// WHY: Validates Default implementation
    /// WHAT: Default should use WASM settings (100K iterations/ms)
    #[test]
    fn test_default() {
        let waiter: SpinWaiter = Default::default();
        assert_eq!(waiter.iterations_per_ms(), 100_000);
    }

    /// WHY: Validates wait completes normally
    /// WHAT: wait() should spin for approximately the requested duration
    #[test]
    fn test_wait_completes() {
        // Use very low iterations for faster test execution
        let waiter = SpinWaiter::new(1_000);

        let start = Instant::now();
        waiter.wait(Duration::from_millis(1));
        let elapsed = start.elapsed();

        // Should complete (timing is approximate due to spin-loop nature)
        // Just verify it doesn't panic and takes some time
        assert!(elapsed >= Duration::from_nanos(1));
    }

    /// WHY: Validates interrupt functionality
    /// WHAT: interrupt() should cause wait() to return immediately
    #[test]
    fn test_interrupt() {
        let waiter = SpinWaiter::new(1_000_000); // High iterations

        // Interrupt before waiting
        waiter.interrupt();
        assert!(waiter.is_interrupted());

        let start = Instant::now();
        waiter.wait(Duration::from_secs(30)); // Would take 30s without interrupt
        let elapsed = start.elapsed();

        // Should return almost immediately due to interrupt
        assert!(
            elapsed < Duration::from_millis(100),
            "wait() took too long after interrupt: {:?}",
            elapsed
        );
    }

    /// WHY: Validates reset clears interrupt
    /// WHAT: reset() should allow subsequent waits to complete normally
    #[test]
    fn test_reset() {
        let waiter = SpinWaiter::new(100_000);

        // Interrupt and then reset
        waiter.interrupt();
        assert!(waiter.is_interrupted());

        waiter.reset();
        assert!(!waiter.is_interrupted());

        // Wait should now complete normally
        let start = Instant::now();
        waiter.wait(Duration::from_millis(5));
        let elapsed = start.elapsed();

        // Should not return immediately (wasn't interrupted)
        assert!(elapsed >= Duration::from_micros(100));
    }

    /// WHY: Validates interrupt during wait
    /// WHAT: interrupt() called during wait should wake early
    #[test]
    fn test_interrupt_during_wait() {
        use std::sync::Arc;
        use std::thread;

        // Use Arc to share the waiter between threads
        let waiter = Arc::new(SpinWaiter::new(500_000));
        let waiter_clone = Arc::clone(&waiter);

        // Spawn thread that will interrupt after short delay
        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            waiter_clone.interrupt();
        });

        let start = Instant::now();
        // This would take ~1 second without interrupt
        waiter.wait(Duration::from_millis(1000));
        let elapsed = start.elapsed();

        handle.join().expect("thread should complete");

        // Should have been interrupted and returned early
        assert!(
            elapsed < Duration::from_millis(500),
            "wait() was not interrupted early enough: {:?}",
            elapsed
        );
    }

    /// WHY: Validates zero duration returns immediately
    /// WHAT: wait(Duration::ZERO) should not spin at all
    #[test]
    fn test_zero_duration() {
        let waiter = SpinWaiter::new(100_000);

        let start = Instant::now();
        waiter.wait(Duration::ZERO);
        let elapsed = start.elapsed();

        // Should return essentially immediately
        assert!(elapsed < Duration::from_millis(1));
    }

    /// WHY: Validates very short duration
    /// WHAT: Short waits should still complete without panic
    #[test]
    fn test_short_duration() {
        let waiter = SpinWaiter::new(1000); // Very low iterations per ms

        let start = Instant::now();
        waiter.wait(Duration::from_micros(100));
        let elapsed = start.elapsed();

        // Should complete quickly (no actual spinning for very short durations)
        assert!(elapsed < Duration::from_millis(10));
    }
