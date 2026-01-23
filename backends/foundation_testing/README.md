# Foundation Testing

Reusable stress testing infrastructure for Foundation synchronization primitives.

## Features

- **Stress Test Framework**: Configurable high-contention testing with thread count, iteration, and duration controls
- **Common Scenarios**: Producer-consumer queues, barriers, thread pools
- **Performance Metrics**: Latency, throughput, and scalability measurements
- **Criterion Benchmarks**: Comparative performance testing

## Usage

### Stress Testing

```rust
use foundation_testing::stress::{StressConfig, sync::run_condvar_stress_test};

let config = StressConfig::new()
    .threads(10)
    .iterations(1000)
    .duration_secs(5);

let result = run_condvar_stress_test(config);
assert!(result.success_rate() > 0.99);
```

### Scenarios

```rust
use foundation_testing::scenarios::ProducerConsumerQueue;
use std::thread;

let queue = ProducerConsumerQueue::new(10);

// Producer
let queue_clone = queue.clone();
thread::spawn(move || {
    for i in 0..100 {
        queue_clone.push(i);
    }
});

// Consumer
for _ in 0..100 {
    let item = queue.pop();
    println!("Got: {}", item);
}
```

### Benchmarking

```bash
cargo bench
```

## Modules

- `stress`: Stress test framework and CondVar-specific tests
- `scenarios`: Common synchronization patterns (producer-consumer, barriers, thread pools)
- `metrics`: Performance metrics collection and reporting

## Dependencies

- `foundation_nostd`: Foundation synchronization primitives
- `criterion`: Benchmarking framework (dev-dependency)

## License

MIT OR Apache-2.0
