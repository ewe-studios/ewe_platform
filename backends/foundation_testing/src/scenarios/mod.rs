//! Common synchronization patterns and scenarios.
//!
//! Provides reusable implementations of classic concurrency patterns:
//! - Producer-consumer queues
//! - Barriers
//! - Thread pools

pub mod producer_consumer;
pub mod barrier;
pub mod thread_pool;

pub use producer_consumer::ProducerConsumerQueue;
pub use barrier::Barrier;
pub use thread_pool::ThreadPool;
