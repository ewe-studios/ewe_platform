//! Native log level mapping tests moved out of `src/log.rs` (spec 70
//! feature 59, rule 7).
//!
//! `tracing_level_for` and its module were private; they are now public.

use infrastructure_llama_bindings as bindings;
use infrastructure_llama_cpp::log::tracing_level_for;
use tracing_core::Level;

#[test]
fn native_info_is_demoted_so_a_plain_info_filter_stays_quiet() {
    // The model-loader dump arrives at ggml INFO. If this ever maps back to
    // tracing INFO, `RUST_LOG=info` floods again.
    assert_eq!(
        tracing_level_for(bindings::GGML_LOG_LEVEL_INFO),
        Level::DEBUG
    );
    assert_eq!(
        tracing_level_for(bindings::GGML_LOG_LEVEL_DEBUG),
        Level::TRACE
    );
}

#[test]
fn native_errors_are_never_demoted() {
    // A caller that quietens this crate must still hear about real faults.
    assert_eq!(
        tracing_level_for(bindings::GGML_LOG_LEVEL_ERROR),
        Level::ERROR
    );
}

#[test]
fn native_warn_is_demoted_to_debug() {
    // Token-type warnings and similar model-metadata quirks are not
    // actionable for the end user. They must not survive a plain `info`
    // filter.
    assert_eq!(
        tracing_level_for(bindings::GGML_LOG_LEVEL_WARN),
        Level::DEBUG
    );
}
