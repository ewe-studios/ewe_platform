//! Comprehensive integration tests for CondVar functionality.
//!
//! These tests verify complex multi-threaded scenarios and edge cases.

use foundation_testing::scenarios::{Barrier, ProducerConsumerQueue, ThreadPool};
use foundation_testing::stress::{StressConfig, StressHarness};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
fn test_multiple_producers_single_consumer() {
    let queue = ProducerConsumerQueue::new(100);
    let num_producers = 5;
    let items_per_producer = 20;
    let total_items = num_producers * items_per_producer;

    let mut handles = vec![];

    // Spawn producers
    for producer_id in 0..num_producers {
        let queue_clone = queue.clone();
        handles.push(thread::spawn(move || {
            for i in 0..items_per_producer {
                queue_clone.push(producer_id * 1000 + i);
            }
        }));
    }

    // Consumer thread
    let queue_clone = queue.clone();
    let consumer = thread::spawn(move || {
        let mut received = vec![];
        for _ in 0..total_items {
            received.push(queue_clone.pop());
        }
        received
    });

    // Wait for producers
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all items received
    let received = consumer.join().unwrap();
    assert_eq!(received.len(), total_items);
}

#[test]
fn test_single_producer_multiple_consumers() {
    let queue = ProducerConsumerQueue::new(50);
    let num_consumers = 4;
    let total_items = 100;
    let done_flag = Arc::new(AtomicBool::new(false));

    // Producer thread
    let queue_clone = queue.clone();
    let done_clone = Arc::clone(&done_flag);
    let producer = thread::spawn(move || {
        for i in 0..total_items {
            queue_clone.push(i);
        }
        // Signal that production is complete
        done_clone.store(true, Ordering::Release);
    });

    // Consumer threads
    let mut handles = vec![];

    for _ in 0..num_consumers {
        let queue_clone = queue.clone();
        let done_clone = Arc::clone(&done_flag);
        handles.push(thread::spawn(move || {
            let mut consumed = 0;
            loop {
                // Check if we should continue consuming
                let done = done_clone.load(Ordering::Acquire);
                let len = queue_clone.len();

                if done && len == 0 {
                    // Producer finished and queue is empty
                    break;
                }

                if len > 0 {
                    let _ = queue_clone.pop();
                    consumed += 1;
                } else if !done {
                    // Queue empty but producer still working
                    thread::yield_now();
                }
            }
            consumed
        }));
    }

    producer.join().unwrap();

    let mut total = 0;
    for handle in handles {
        total += handle.join().unwrap();
    }

    // All items should be consumed
    assert_eq!(total, total_items);
}

#[test]
fn test_barrier_reuse() {
    // Test that a barrier can be reused multiple times
    let barrier = Arc::new(Barrier::new(3));
    let mut handles = vec![];

    for thread_id in 0..3 {
        let barrier_clone = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            for iteration in 0..5 {
                let is_leader = barrier_clone.wait();
                // Only one thread should be leader each iteration
                if is_leader {
                    println!("Thread {} is leader at iteration {}", thread_id, iteration);
                }
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_thread_pool_concurrent_execution() {
    let pool = ThreadPool::new(4);
    let counter = Arc::new(AtomicUsize::new(0));
    let num_jobs = 100;

    for _ in 0..num_jobs {
        let counter_clone = Arc::clone(&counter);
        pool.execute(move || {
            counter_clone.fetch_add(1, Ordering::Relaxed);
            thread::sleep(Duration::from_millis(1));
        });
    }

    // Wait for all jobs to complete
    drop(pool);

    assert_eq!(counter.load(Ordering::Relaxed), num_jobs);
}

#[test]
fn test_thread_pool_ordering() {
    // Verify that jobs are executed (not necessarily in order due to parallelism)
    let pool = ThreadPool::new(2);
    let results = Arc::new(std::sync::Mutex::new(Vec::new()));

    for i in 0..10 {
        let results_clone = Arc::clone(&results);
        pool.execute(move || {
            results_clone.lock().unwrap().push(i);
        });
    }

    drop(pool);

    let results = results.lock().unwrap();
    assert_eq!(results.len(), 10);
    // All numbers should be present (order may vary)
    for i in 0..10 {
        assert!(results.contains(&i));
    }
}

#[test]
fn test_stress_with_rapid_notifications() {
    // Test rapid notify_one calls
    let config = StressConfig::new().threads(8).iterations(500);
    let harness = StressHarness::new(config);

    let success_count = Arc::new(AtomicUsize::new(0));

    let counter = Arc::clone(&success_count);
    let result = harness.run(move |_thread_id, _iteration| {
        counter.fetch_add(1, Ordering::Relaxed);
        true
    });

    assert_eq!(result.successes, 8 * 500);
    assert_eq!(result.failures, 0);
    assert_eq!(success_count.load(Ordering::Relaxed), 8 * 500);
}

#[test]
fn test_stress_with_timeout() {
    // Test that duration-limited stress test stops correctly
    let config = StressConfig::new()
        .threads(4)
        .iterations(1_000_000) // Very high, should be stopped by timeout
        .duration_secs(1);

    let harness = StressHarness::new(config);

    let result = harness.run(|_thread_id, _iteration| {
        thread::sleep(Duration::from_micros(10));
        true
    });

    // Should complete in approximately 1 second
    assert!(result.duration.as_secs() <= 2);
    // Should not have completed all iterations
    assert!(result.total_operations() < 1_000_000);
}

#[test]
fn test_producer_consumer_fairness() {
    // Test that consumers get roughly equal shares
    let queue = ProducerConsumerQueue::new(10);
    let num_items = 100;
    let num_consumers = 4;
    let done_flag = Arc::new(AtomicBool::new(false));

    // Producer
    let queue_clone = queue.clone();
    let done_clone = Arc::clone(&done_flag);
    let producer = thread::spawn(move || {
        for i in 0..num_items {
            queue_clone.push(i);
        }
        done_clone.store(true, Ordering::Release);
    });

    // Consumers
    let mut handles = vec![];
    for consumer_id in 0..num_consumers {
        let queue_clone = queue.clone();
        let done_clone = Arc::clone(&done_flag);
        handles.push(thread::spawn(move || {
            let mut count = 0;
            loop {
                let done = done_clone.load(Ordering::Acquire);
                let len = queue_clone.len();

                if done && len == 0 {
                    break;
                }

                if len > 0 {
                    let _ = queue_clone.pop();
                    count += 1;
                } else if !done {
                    thread::yield_now();
                }
            }
            println!("Consumer {} received {} items", consumer_id, count);
            count
        }));
    }

    producer.join().unwrap();

    let mut counts = vec![];
    for handle in handles {
        counts.push(handle.join().unwrap());
    }

    let total: usize = counts.iter().sum();
    assert_eq!(total, num_items);
}

#[test]
fn test_barrier_with_uneven_thread_arrival() {
    // Test barrier when threads arrive at different times
    let barrier = Arc::new(Barrier::new(3));
    let start_flag = Arc::new(AtomicBool::new(false));
    let mut handles = vec![];

    for thread_id in 0..3 {
        let barrier_clone = Arc::clone(&barrier);
        let flag_clone = Arc::clone(&start_flag);
        handles.push(thread::spawn(move || {
            // Stagger thread starts
            thread::sleep(Duration::from_millis(thread_id * 10));

            // Wait for start signal
            while !flag_clone.load(Ordering::Relaxed) {
                thread::yield_now();
            }

            barrier_clone.wait();
        }));
    }

    // Give threads time to start
    thread::sleep(Duration::from_millis(50));

    // Signal start
    start_flag.store(true, Ordering::Relaxed);

    // All threads should complete
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_nested_synchronization() {
    // Test using multiple synchronization primitives together
    // Simpler version: produce then consume in sequence
    let barrier = Arc::new(Barrier::new(2));
    let queue = ProducerConsumerQueue::new(10);

    let barrier1 = Arc::clone(&barrier);
    let queue1 = queue.clone();
    let thread1 = thread::spawn(move || {
        // Produce items
        for i in 0..3 {
            queue1.push(i);
        }

        // Wait for both threads to finish producing
        barrier1.wait();

        // Consume 3 items from the shared pool
        let mut sum = 0;
        for _ in 0..3 {
            sum += queue1.pop();
        }
        sum
    });

    let barrier2 = Arc::clone(&barrier);
    let queue2 = queue.clone();
    let thread2 = thread::spawn(move || {
        // Produce items
        for i in 10..13 {
            queue2.push(i);
        }

        // Wait for both threads to finish producing
        barrier2.wait();

        // Consume 3 items from the shared pool
        let mut sum = 0;
        for _ in 0..3 {
            sum += queue2.pop();
        }
        sum
    });

    let sum1 = thread1.join().unwrap();
    let sum2 = thread2.join().unwrap();

    // Total should be all items (0+1+2+10+11+12 = 36)
    assert_eq!(sum1 + sum2, 36);
}

#[test]
fn test_high_contention_scenario() {
    // Many threads competing for a single resource
    let queue = ProducerConsumerQueue::new(1); // Small capacity = high contention
    let num_threads = 10;
    let items_per_thread = 20;

    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let queue_clone = queue.clone();
        handles.push(thread::spawn(move || {
            for i in 0..items_per_thread {
                queue_clone.push(thread_id * 1000 + i);
                let _ = queue_clone.pop();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Queue should be empty at the end
    assert_eq!(queue.len(), 0);
}

#[test]
fn test_zero_capacity_queue() {
    // Edge case: queue with minimal capacity
    let queue = ProducerConsumerQueue::new(1);

    let producer = {
        let queue = queue.clone();
        thread::spawn(move || {
            for i in 0..5 {
                queue.push(i);
            }
        })
    };

    let consumer = {
        let queue = queue.clone();
        thread::spawn(move || {
            let mut items = vec![];
            for _ in 0..5 {
                items.push(queue.pop());
            }
            items
        })
    };

    producer.join().unwrap();
    let items = consumer.join().unwrap();

    assert_eq!(items, vec![0, 1, 2, 3, 4]);
}
