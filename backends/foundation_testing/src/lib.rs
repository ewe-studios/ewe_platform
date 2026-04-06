//! Reusable testing infrastructure for Foundation crates.
//!
//! This crate provides:
//! - **HTTP test server**: Real HTTP server built on `foundation_core`'s `simple_http` types
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
#![allow(clippy::pedantic)]
#![allow(dead_code)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::similar_names)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::needless_continue)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::unnested_or_patterns)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::type_complexity)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

pub mod http;
pub mod io;
pub mod metrics;
pub mod netcap;
pub mod scenarios;
pub mod stress;

// HuggingFace model downloading (optional feature)
#[cfg(feature = "huggingface")]
pub mod huggingface;

// Re-export commonly used items
pub use http::TestHttpServer;
pub use io::{SharedBuffer, SharedBufferReader, SharedBufferWriter};
pub use metrics::{Metrics, PerformanceReport};
pub use netcap::ResourcesHttpServer;
pub use stress::{StressConfig, StressHarness, StressResult};

// Re-export huggingface items when feature is enabled
#[cfg(feature = "huggingface")]
pub use huggingface::{TestHarness, DEFAULT_ARTIFACTS_DIR};
