//! WHY: Unified logging API for the Ewe platform with flexible backends.
//!
//! WHAT: Provides logging macros that use `tracing` when the `tracing` feature is enabled,
//! otherwise falls back to `println!` for simple console output.
//!
//! HOW: Uses conditional compilation based on the `tracing` feature flag.
//!
//! # Usage
//!
//! ```rust,ignore
//! use foundation_logging::{info, debug, warn, error, trace};
//!
//! info!("Something happened");
//! info!(target: "my_target", "Something happened with extra: {}", extra);
//! debug!("Debug message");
//! ```
//!
//! # Feature Flags
//!
//! - `tracing`: When enabled, uses the `tracing` crate for structured logging.
//!   Otherwise, falls back to `println!` based console logging.

#![cfg_attr(not(feature = "tracing"), allow(unused_macros))]

/// Log an info message.
///
/// When the `tracing` feature is enabled, this expands to `tracing::info!`.
/// Otherwise, it expands to a `println!` based implementation.
#[macro_export]
macro_rules! info {
    (target: $target:expr, $($arg:tt)+) => {
        $crate::info_impl!($target, $($arg)+);
    };
    ($($arg:tt)+) => {
        $crate::info_impl!("default", $($arg)+);
    };
}

/// Log a debug message.
///
/// When the `tracing` feature is enabled, this expands to `tracing::debug!`.
/// Otherwise, it expands to a `println!` based implementation.
#[macro_export]
macro_rules! debug {
    (target: $target:expr, $($arg:tt)+) => {
        $crate::debug_impl!($target, $($arg)+);
    };
    ($($arg:tt)+) => {
        $crate::debug_impl!("default", $($arg)+);
    };
}

/// Log a warn message.
///
/// When the `tracing` feature is enabled, this expands to `tracing::warn!`.
/// Otherwise, it expands to a `println!` based implementation.
#[macro_export]
macro_rules! warn {
    (target: $target:expr, $($arg:tt)+) => {
        $crate::warn_impl!($target, $($arg)+);
    };
    ($($arg:tt)+) => {
        $crate::warn_impl!("default", $($arg)+);
    };
}

/// Log an error message.
///
/// When the `tracing` feature is enabled, this expands to `tracing::error!`.
/// Otherwise, it expands to a `println!` based implementation.
#[macro_export]
macro_rules! error {
    (target: $target:expr, $($arg:tt)+) => {
        $crate::error_impl!($target, $($arg)+);
    };
    ($($arg:tt)+) => {
        $crate::error_impl!("default", $($arg)+);
    };
}

/// Log a trace message.
///
/// When the `tracing` feature is enabled, this expands to `tracing::trace!`.
/// Otherwise, it expands to a `println!` based implementation.
#[macro_export]
macro_rules! trace {
    (target: $target:expr, $($arg:tt)+) => {
        $crate::trace_impl!($target, $($arg)+);
    };
    ($($arg:tt)+) => {
        $crate::trace_impl!("default", $($arg)+);
    };
}

/// Internal implementation details - not for direct use.
#[doc(hidden)]
pub mod internal {}

// Implementation macros - exported at crate root due to #[macro_export]
// When tracing feature is enabled, define impl macros that delegate to tracing
#[cfg(feature = "tracing")]
#[macro_export]
macro_rules! info_impl {
    ($target:expr, $($arg:tt)+) => {
        tracing::info!(target: $target, $($arg)+);
    };
}

#[cfg(feature = "tracing")]
#[macro_export]
macro_rules! debug_impl {
    ($target:expr, $($arg:tt)+) => {
        tracing::debug!(target: $target, $($arg)+);
    };
}

#[cfg(feature = "tracing")]
#[macro_export]
macro_rules! warn_impl {
    ($target:expr, $($arg:tt)+) => {
        tracing::warn!(target: $target, $($arg)+);
    };
}

#[cfg(feature = "tracing")]
#[macro_export]
macro_rules! error_impl {
    ($target:expr, $($arg:tt)+) => {
        tracing::error!(target: $target, $($arg)+);
    };
}

#[cfg(feature = "tracing")]
#[macro_export]
macro_rules! trace_impl {
    ($target:expr, $($arg:tt)+) => {
        tracing::trace!(target: $target, $($arg)+);
    };
}

// When tracing is not enabled, use println! based implementations
#[cfg(not(feature = "tracing"))]
#[macro_export]
macro_rules! info_impl {
    ($_target:expr, $($arg:tt)+) => {
        println!("[INFO] {}", format!($($arg)+));
    };
}

#[cfg(not(feature = "tracing"))]
#[macro_export]
macro_rules! debug_impl {
    ($_target:expr, $($arg:tt)+) => {
        println!("[DEBUG] {}", format!($($arg)+));
    };
}

#[cfg(not(feature = "tracing"))]
#[macro_export]
macro_rules! warn_impl {
    ($_target:expr, $($arg:tt)+) => {
        println!("[WARN] {}", format!($($arg)+));
    };
}

#[cfg(not(feature = "tracing"))]
#[macro_export]
macro_rules! error_impl {
    ($_target:expr, $($arg:tt)+) => {
        println!("[ERROR] {}", format!($($arg)+));
    };
}

#[cfg(not(feature = "tracing"))]
#[macro_export]
macro_rules! trace_impl {
    ($_target:expr, $($arg:tt)+) => {
        println!("[TRACE] {}", format!($($arg)+));
    };
}

/// Initialize the logging system.
///
/// When the `tracing` feature is enabled, this sets up a tracing subscriber.
/// Otherwise, this is a no-op since println! doesn't need initialization.
///
/// # Example
///
/// ```rust,ignore
/// use foundation_logging::init;
///
/// #[tokio::main]
/// async fn main() {
///     init();
///     // ... rest of your code
/// }
/// ```
pub fn init() {
    #[cfg(feature = "tracing")]
    {
        use tracing_subscriber::EnvFilter;

        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        tracing_subscriber::fmt()
            .with_target(false)
            .with_thread_ids(false)
            .with_thread_names(false)
            .with_level(true)
            .with_env_filter(filter)
            .init();
    }

    #[cfg(not(feature = "tracing"))]
    {
        // No-op for println! based logging
    }
}

/// Initialize the logging system with a custom filter.
///
/// # Arguments
///
/// * `filter` - A string specifying the log filter (e.g., "debug", "info", "my_crate=debug")
#[cfg(feature = "tracing")]
pub fn init_with_filter(filter: &str) {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_new(filter).unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_level(true)
        .with_env_filter(filter)
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(feature = "tracing"))]
    fn test_info_macro_no_tracing() {
        // Just verify the macro compiles and runs without panic
        info!("Test info message");
        info!(target: "test_target", "Test info with target");
    }

    #[test]
    #[cfg(not(feature = "tracing"))]
    fn test_debug_macro_no_tracing() {
        debug!("Test debug message");
        debug!(target: "test_target", "Test debug with target");
    }

    #[test]
    #[cfg(not(feature = "tracing"))]
    fn test_warn_macro_no_tracing() {
        warn!("Test warn message");
        warn!(target: "test_target", "Test warn with target");
    }

    #[test]
    #[cfg(not(feature = "tracing"))]
    fn test_error_macro_no_tracing() {
        error!("Test error message");
        error!(target: "test_target", "Test error with target");
    }

    #[test]
    #[cfg(not(feature = "tracing"))]
    fn test_trace_macro_no_tracing() {
        trace!("Test trace message");
        trace!(target: "test_target", "Test trace with target");
    }

    #[test]
    #[cfg(not(feature = "tracing"))]
    fn test_info_with_args_no_tracing() {
        info!("Test info with args: {} {} {}", 42, "hello", 3.54f32);
    }

    #[test]
    #[cfg(feature = "tracing")]
    fn test_info_macro_with_tracing() {
        // Initialize tracing for tests
        let _ = tracing_subscriber::fmt()
            .with_target(false)
            .with_thread_ids(false)
            .with_thread_names(false)
            .with_level(false)
            .with_test_writer()
            .try_init();

        info!("Test info message");
        info!(target: "test_target", "Test info with target");
    }

    #[test]
    #[cfg(feature = "tracing")]
    fn test_debug_macro_with_tracing() {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

        debug!("Test debug message");
        debug!(target: "test_target", "Test debug with target");
    }

    #[test]
    #[cfg(feature = "tracing")]
    fn test_warn_macro_with_tracing() {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

        warn!("Test warn message");
        warn!(target: "test_target", "Test warn with target");
    }

    #[test]
    #[cfg(feature = "tracing")]
    fn test_error_macro_with_tracing() {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

        error!("Test error message");
        error!(target: "test_target", "Test error with target");
    }

    #[test]
    #[cfg(feature = "tracing")]
    fn test_trace_macro_with_tracing() {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

        trace!("Test trace message");
        trace!(target: "test_target", "Test trace with target");
    }

    #[test]
    #[cfg(feature = "tracing")]
    fn test_info_with_args_with_tracing() {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();

        info!("Test info with args: {} {} {}", 42, "hello", 3.14);
    }

    #[test]
    #[cfg(not(feature = "tracing"))]
    fn test_init_no_tracing_is_noop() {
        // Should not panic
        init();
    }

    // Note: We don't test init() with tracing feature because:
    // 1. tracing_subscriber can only be initialized once per process
    // 2. Tests run in parallel and would conflict
    // The macro tests above verify tracing integration works correctly
}
