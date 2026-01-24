//! Integration tests for foundation_testing crate.

#[cfg(test)]
mod tests {
    use crate::stress::{StressConfig, StressHarness, sync::run_condvar_stress_test};
    use crate::scenarios::{ProducerConsumerQueue, Barrier, ThreadPool};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    #[test]
    fn test_stress_harness_basic() {
        let config = StressConfig::new().threads(2).iterations(10);
        let harness = StressHarness::new(config);

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        let result = harness.run(move |_thread_id, _iteration| {
            counter_clone.fetch_add(1, Ordering::Relaxed);
            true
        });

        assert_eq!(result.successes, 20); // 2 threads * 10 iterations
        assert_eq!(result.failures, 0);
        assert_eq!(counter.load(Ordering::Relaxed), 20);
    }

    #[test]
    fn test_condvar_stress_test() {
        let config = StressConfig::new().threads(4).iterations(50);
        let result = run_condvar_stress_test(config);

        assert!(result.success_rate() > 0.99);
        assert_eq!(result.total_operations(), 200); // 4 * 50
    }

    #[test]
    fn test_producer_consumer_queue() {
        let queue = ProducerConsumerQueue::new(10);

        let queue_clone = queue.clone();
        let producer = thread::spawn(move || {
            for i in 0..5 {
                queue_clone.push(i);
            }
        });

        let queue_clone = queue.clone();
        let consumer = thread::spawn(move || {
            let mut sum = 0;
            for _ in 0..5 {
                sum += queue_clone.pop();
            }
            sum
        });

        producer.join().unwrap();
        let sum = consumer.join().unwrap();

        assert_eq!(sum, 0 + 1 + 2 + 3 + 4); // 10
    }

    #[test]
    fn test_barrier() {
        let barrier = Arc::new(Barrier::new(3));
        let counter = Arc::new(AtomicUsize::new(0));
        let mut handles = vec![];

        for _ in 0..3 {
            let barrier_clone = Arc::clone(&barrier);
            let counter_clone = Arc::clone(&counter);
            handles.push(thread::spawn(move || {
                counter_clone.fetch_add(1, Ordering::Relaxed);
                barrier_clone.wait();
                counter_clone.load(Ordering::Relaxed)
            }));
        }

        let results: Vec<usize> = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect();

        // All threads should see count == 3 after barrier
        for count in results {
            assert_eq!(count, 3);
        }
    }

    #[test]
    fn test_thread_pool() {
        let pool = ThreadPool::new(4);
        let counter = Arc::new(AtomicUsize::new(0));

        for _ in 0..10 {
            let counter_clone = Arc::clone(&counter);
            pool.execute(move || {
                counter_clone.fetch_add(1, Ordering::Relaxed);
            });
        }

        // Drop pool to wait for all jobs
        drop(pool);

        assert_eq!(counter.load(Ordering::Relaxed), 10);
    }
}
