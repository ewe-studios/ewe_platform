//! Reusable testing infrastructure for Foundation crates.
//!
//! This crate provides:
//! - **HTTP test server**: Real HTTP server built on foundation_core's simple_http types
//! - **Stress test framework**: Configurable high-contention testing
//! - **Common scenarios**: Producer-consumer, barriers, thread pools
//! - **Performance metrics**: Latency, throughput, scalability measurements
//! - **Criterion benchmarks**: Comparative performance testing
//!
//! # Examples
//!
//! ## HTTP Test Server
//!
//! ```rust
//! use foundation_testing::http::TestHttpServer;
//!
//! let server = TestHttpServer::start();
//! // Use server.url("/") in your HTTP client tests
//! ```
//!
//! ## Stress Testing
//!
//! ```rust
//! use foundation_testing::stress::{StressConfig, StressHarness};
//! use std::sync::atomic::{AtomicUsize, Ordering};
//! use std::sync::Arc;
//!
//! let config = StressConfig::new()
//!     .threads(10)
//!     .iterations(1000);
//!
//! let counter = Arc::new(AtomicUsize::new(0));
//! let harness = StressHarness::new(config);
//!
//! let counter_clone = Arc::clone(&counter);
//! let results = harness.run(move |_thread_id, _iteration| {
//!     counter_clone.fetch_add(1, Ordering::Relaxed);
//!     true
//! });
//!
//! assert_eq!(results.successes, 10000); // 10 threads * 1000 iterations
//! assert!(results.success_rate() > 0.99);
//! ```
//!
//! # Features
//!
//! - `std` (default): Enables std features including threading

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)] // Common for testing crates

pub mod http;
pub mod metrics;
pub mod scenarios;
pub mod stress;

// Re-export commonly used items
pub use http::TestHttpServer;
pub use metrics::{Metrics, PerformanceReport};
pub use stress::{StressConfig, StressHarness, StressResult};
