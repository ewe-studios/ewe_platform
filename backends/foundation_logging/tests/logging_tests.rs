//! Logging macro tests moved out of `src/lib.rs` (spec 70 feature 59, rule 7).
//!
//! The macros are `#[macro_export]`, so the test uses only the public crate
//! root. The `tracing` feature selects which half of the suite compiles.

use foundation_logging::{debug, error, info, trace, warn};

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

    info!(
        "Test info with args: {} {} {}",
        42,
        "hello",
        std::f32::consts::PI
    );
}

#[test]
#[cfg(not(feature = "tracing"))]
fn test_init_no_tracing_is_noop() {
    // Should not panic
    foundation_logging::init();
}

// Note: We don't test init() with tracing feature because:
// 1. tracing_subscriber can only be initialized once per process
// 2. Tests run in parallel and would conflict
// The macro tests above verify tracing integration works correctly
