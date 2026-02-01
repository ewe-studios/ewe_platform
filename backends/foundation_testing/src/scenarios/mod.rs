//! Common synchronization patterns and scenarios.
//!
//! Provides reusable implementations of classic concurrency patterns:
//! - Producer-consumer queues
//! - Barriers
//! - Thread pools

pub mod barrier;
pub mod producer_consumer;
pub mod thread_pool;

pub use barrier::Barrier;
pub use producer_consumer::ProducerConsumerQueue;
pub use thread_pool::ThreadPool;
