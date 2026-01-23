use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use foundation_nostd::primitives::{CondVar, CondVarMutex, CondVarNonPoisoning, RawCondVarMutex};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Benchmark uncontended wait/notify latency.
fn bench_condvar_uncontended(c: &mut Criterion) {
    c.bench_function("condvar_wait_notify_uncontended", |b| {
        b.iter_batched(
            || {
                let mutex = Arc::new(CondVarMutex::new(false));
                let condvar = Arc::new(CondVar::new());
                (mutex, condvar)
            },
            |(mutex, condvar)| {
                let mutex_clone = Arc::clone(&mutex);
                let condvar_clone = Arc::clone(&condvar);

                // Spawn waiter thread
                let waiter = thread::spawn(move || {
                    let mut guard = mutex_clone.lock().unwrap();
                    while !*guard {
                        guard = condvar_clone.wait(guard).unwrap();
                    }
                });

                // Small delay to ensure waiter is waiting
                thread::sleep(Duration::from_micros(100));

                // Notify
                *mutex.lock().unwrap() = true;
                condvar.notify_one();

                waiter.join().unwrap();
            },
            BatchSize::SmallInput,
        );
    });
}

/// Benchmark notify_one with multiple waiters.
fn bench_condvar_notify_one_contended(c: &mut Criterion) {
    c.bench_function("condvar_notify_one_10_waiters", |b| {
        b.iter_batched(
            || {
                let mutex = Arc::new(CondVarMutex::new(0u32));
                let condvar = Arc::new(CondVar::new());
                (mutex, condvar)
            },
            |(mutex, condvar)| {
                let mut waiters = vec![];

                // Spawn 10 waiters
                for _ in 0..10 {
                    let mutex_clone = Arc::clone(&mutex);
                    let condvar_clone = Arc::clone(&condvar);

                    waiters.push(thread::spawn(move || {
                        let mut guard = mutex_clone.lock().unwrap();
                        while *guard == 0 {
                            guard = condvar_clone.wait(guard).unwrap();
                        }
                    }));
                }

                // Small delay
                thread::sleep(Duration::from_millis(10));

                // Notify all (one at a time)
                for _ in 0..10 {
                    *mutex.lock().unwrap() += 1;
                    condvar.notify_one();
                    thread::sleep(Duration::from_micros(100));
                }

                for waiter in waiters {
                    waiter.join().unwrap();
                }
            },
            BatchSize::SmallInput,
        );
    });
}

/// Benchmark notify_all scaling.
fn bench_condvar_notify_all_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("condvar_notify_all_scaling");

    for num_threads in [10, 50, 100] {
        group.bench_function(format!("{}_threads", num_threads), |b| {
            b.iter_batched(
                || {
                    let mutex = Arc::new(CondVarMutex::new(false));
                    let condvar = Arc::new(CondVar::new());
                    (mutex, condvar)
                },
                |(mutex, condvar)| {
                    let mut waiters = vec![];

                    // Spawn waiters
                    for _ in 0..num_threads {
                        let mutex_clone = Arc::clone(&mutex);
                        let condvar_clone = Arc::clone(&condvar);

                        waiters.push(thread::spawn(move || {
                            let mut guard = mutex_clone.lock().unwrap();
                            while !*guard {
                                guard = condvar_clone.wait(guard).unwrap();
                            }
                        }));
                    }

                    // Small delay
                    thread::sleep(Duration::from_millis(50));

                    // Notify all
                    *mutex.lock().unwrap() = true;
                    condvar.notify_all();

                    for waiter in waiters {
                        waiter.join().unwrap();
                    }
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark wait_timeout accuracy.
fn bench_condvar_timeout(c: &mut Criterion) {
    c.bench_function("condvar_wait_timeout_100us", |b| {
        let mutex = Arc::new(CondVarMutex::new(0u32));
        let condvar = Arc::new(CondVar::new());

        b.iter(|| {
            let guard = mutex.lock().unwrap();
            let (_guard, result) = condvar
                .wait_timeout(guard, Duration::from_micros(100))
                .unwrap();
            black_box(result.timed_out());
        });
    });
}

/// Compare CondVar vs CondVarNonPoisoning performance.
fn bench_condvar_vs_nonpoisoning(c: &mut Criterion) {
    let mut group = c.benchmark_group("condvar_poisoning_comparison");

    group.bench_function("with_poisoning", |b| {
        let mutex = Arc::new(CondVarMutex::new(0u32));
        let condvar = Arc::new(CondVar::new());

        b.iter(|| {
            let guard = mutex.lock().unwrap();
            let (_guard, result) = condvar
                .wait_timeout(guard, Duration::from_micros(10))
                .unwrap();
            black_box(result.timed_out());
        });
    });

    group.bench_function("without_poisoning", |b| {
        let mutex = Arc::new(RawCondVarMutex::new(0u32));
        let condvar = Arc::new(CondVarNonPoisoning::new());

        b.iter(|| {
            let guard = mutex.lock();
            let (_guard, result) = condvar.wait_timeout(guard, Duration::from_micros(10));
            black_box(result.timed_out());
        });
    });

    group.finish();
}

/// Benchmark std::sync::Condvar for comparison.
#[cfg(feature = "std")]
fn bench_std_condvar(c: &mut Criterion) {
    use std::sync::{Condvar, Mutex};

    c.bench_function("std_condvar_wait_timeout_100us", |b| {
        let mutex = Arc::new(Mutex::new(0u32));
        let condvar = Arc::new(Condvar::new());

        b.iter(|| {
            let guard = mutex.lock().unwrap();
            let (_guard, result) = condvar
                .wait_timeout(guard, Duration::from_micros(100))
                .unwrap();
            black_box(result.timed_out());
        });
    });
}

criterion_group!(
    benches,
    bench_condvar_uncontended,
    bench_condvar_notify_one_contended,
    bench_condvar_notify_all_scaling,
    bench_condvar_timeout,
    bench_condvar_vs_nonpoisoning,
    #[cfg(feature = "std")]
    bench_std_condvar,
);
criterion_main!(benches);
