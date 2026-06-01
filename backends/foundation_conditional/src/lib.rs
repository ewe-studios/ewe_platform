//! Block-level conditional compilation macros.
//!
//! These macros gate entire code blocks on feature flags without repeating `#[cfg()]` per line.
//! When the feature is not enabled, the block expands to `{}` — the code is never compiled,
//! type-checked, or linked. Zero cost.
//!
//! # Example
//!
//! ```rust
//! use foundation_conditional::debug;
//!
//! debug! {
//!     let start = std::time::Instant::now();
//!     tracing::debug!("processing file: {:?}", path);
//!     let result = expensive_validation(&path);
//!     tracing::debug!("validation took: {:?}", start.elapsed());
//! }
//! ```
//!
//! # Caveat: variable scoping
//!
//! Code after the block cannot reference variables declared inside it, since the block is its
//! own scope. If you need the value outside the block, gate the entire usage.

/// Gates a block of code on the `debug_block` feature.
/// Expands to the block when enabled, or `{}` when disabled.
#[macro_export]
macro_rules! debug {
    ($($block:tt)*) => {
        #[cfg(feature = "debug_block")]
        { $($block)* }
        #[cfg(not(feature = "debug_block"))]
        {}
    };
}

/// Gates a block of code on the `trace_block` feature.
/// Semantically equivalent to [`debug!`] but named for trace-level instrumentation.
#[macro_export]
macro_rules! trace {
    ($($block:tt)*) => {
        #[cfg(feature = "trace_block")]
        { $($block)* }
        #[cfg(not(feature = "trace_block"))]
        {}
    };
}

/// Gates a block of code on the `test_utils` feature.
/// Useful for test-only helper code in non-test functions.
#[macro_export]
macro_rules! test_util {
    ($($block:tt)*) => {
        #[cfg(feature = "test_utils")]
        { $($block)* }
        #[cfg(not(feature = "test_utils"))]
        {}
    };
}

/// General-purpose block-level conditional compilation with explicit feature name.
///
/// # Example
///
/// ```rust
/// use foundation_conditional::cfg_block;
///
/// cfg_block!(my_feature => {
///     // only compiled when `my_feature` is enabled
/// });
/// ```
#[macro_export]
macro_rules! cfg_block {
    ($name:ident => { $($block:tt)* }) => {
        #[cfg(feature = $name)]
        { $($block)* }
        #[cfg(not(feature = $name))]
        {}
    };
}
