use foundation_nostd::primitives::barrier::*;
    #[cfg(not(feature = "std"))]
    use alloc::format;

    /// WHY: Validates barrier construction
    /// WHAT: Creating a barrier with count should work
    #[test]
    fn test_new() {
        let barrier = SpinBarrier::new(3);
        assert_eq!(barrier.num_threads, 3);
    }

    /// WHY: Validates barrier panics on zero count
    /// WHAT: Creating barrier with 0 threads should panic
    #[test]
    #[should_panic(expected = "barrier count must be > 0")]
    fn test_new_zero_panics() {
        let _ = SpinBarrier::new(0);
    }

    /// WHY: Validates single thread barrier
    /// WHAT: Barrier with n=1 should immediately return leader
    #[test]
    fn test_single_thread() {
        let barrier = SpinBarrier::new(1);
        let result = barrier.wait();
        assert!(result.is_leader());
    }

    /// WHY: Validates barrier is reusable
    /// WHAT: Barrier should work for multiple rounds
    #[test]
    fn test_reusable() {
        let barrier = SpinBarrier::new(1);

        // First round
        let result1 = barrier.wait();
        assert!(result1.is_leader());

        // Second round
        let result2 = barrier.wait();
        assert!(result2.is_leader());
    }

    /// WHY: Validates Debug implementation
    /// WHAT: Debug formatting should work
    #[test]
    fn test_debug() {
        let barrier = SpinBarrier::new(3);
        let debug = format!("{barrier:?}");
        assert!(debug.contains("SpinBarrier"));
        assert!(debug.contains("num_threads"));
    }

    /// WHY: A barrier for n > 1 must only release threads once all n have
    /// arrived, and must be reusable across many generations.
    /// WHAT: n threads rendezvous for many rounds; after every wait each
    /// thread must observe all n arrivals of that round, and every round must
    /// have exactly one leader.
    #[test]
    fn test_multi_thread_rounds_release_only_after_all_arrive() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::thread;

        const THREADS: usize = 4;
        const ROUNDS: usize = 500;

        let barrier = Arc::new(SpinBarrier::new(THREADS));
        let arrivals = Arc::new(AtomicUsize::new(0));
        let leaders = Arc::new(AtomicUsize::new(0));

        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let barrier = Arc::clone(&barrier);
                let arrivals = Arc::clone(&arrivals);
                let leaders = Arc::clone(&leaders);
                thread::spawn(move || {
                    for round in 1..=ROUNDS {
                        arrivals.fetch_add(1, Ordering::SeqCst);
                        if barrier.wait().is_leader() {
                            leaders.fetch_add(1, Ordering::SeqCst);
                        }
                        let seen = arrivals.load(Ordering::SeqCst);
                        assert!(
                            seen >= round * THREADS,
                            "released early in round {round}: saw {seen} arrivals"
                        );
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("barrier thread panicked");
        }

        assert_eq!(arrivals.load(Ordering::SeqCst), THREADS * ROUNDS);
        assert_eq!(leaders.load(Ordering::SeqCst), ROUNDS);
    }

    /// WHY: When more threads than the barrier count share it, arrivals must
    /// be grouped into complete generations. Previously the count reset and
    /// the generation bump were separate stores, so a thread arriving between
    /// them recorded the old generation and was released by that bump without
    /// its own generation ever filling up.
    /// WHAT: In each batch, 2*n threads call `wait()` exactly once on a shared
    /// barrier of n (so every generation can fill and nobody is left without
    /// partners). A thread may only return once its generation is full, so the
    /// number of threads released can never exceed the number of complete
    /// generations' worth of arrivals. Each batch must have exactly two
    /// leaders, and the barrier must end with no partially filled generation.
    #[test]
    fn test_more_threads_than_count_forms_complete_generations() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::thread;

        const COUNT: usize = 2;
        const THREADS: usize = 2 * COUNT;
        const BATCHES: usize = 300;

        let barrier = Arc::new(SpinBarrier::new(COUNT));
        let arrivals = Arc::new(AtomicUsize::new(0));
        let released = Arc::new(AtomicUsize::new(0));
        let leaders = Arc::new(AtomicUsize::new(0));

        for batch in 0..BATCHES {
            let handles: Vec<_> = (0..THREADS)
                .map(|_| {
                    let barrier = Arc::clone(&barrier);
                    let arrivals = Arc::clone(&arrivals);
                    let released = Arc::clone(&released);
                    let leaders = Arc::clone(&leaders);
                    thread::spawn(move || {
                        arrivals.fetch_add(1, Ordering::SeqCst);
                        let is_leader = barrier.wait().is_leader();
                        let released_now = released.fetch_add(1, Ordering::SeqCst) + 1;
                        let arrived_now = arrivals.load(Ordering::SeqCst);
                        let complete_slots = (arrived_now / COUNT) * COUNT;
                        assert!(
                            released_now <= complete_slots,
                            "thread released before its generation filled: \
                             {released_now} released, {arrived_now} arrived"
                        );
                        if is_leader {
                            leaders.fetch_add(1, Ordering::SeqCst);
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.join().expect("barrier thread panicked");
            }
            assert_eq!(
                leaders.load(Ordering::SeqCst),
                (batch + 1) * (THREADS / COUNT),
                "wrong number of leaders after batch {batch}"
            );
        }

        assert_eq!(released.load(Ordering::SeqCst), BATCHES * THREADS);
        let debug = format!("{barrier:?}");
        assert!(
            debug.contains("count: 0"),
            "barrier left mid-generation: {debug}"
        );
    }
