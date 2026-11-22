//! A failing wasm case must name itself in the runner's error (spec 88 rule R3,
//! issue #588).
//!
//! WHY: the owned browser run on Linux printed a count and no case name, so the
//! failing case could not be found from the log.
//! WHAT: the error that carries a failed run keeps the whole page text.
//! HOW: builds each error from a page text and reads its message.

use foundation_testbed::wasm::error::WasmTestbedError;

/// A page text with one passing case, one failing case and its message.
const PAGE: &str = "ok      a_passes
FAILED  any_readiness_resumes_when_a_child_is_ready
        any_readiness_park: polled while parked
test result: FAILED. 1 passed; 1 failed; 0 ignored";

#[test]
fn the_owned_failure_keeps_the_failed_case_and_its_message() {
    let error = WasmTestbedError::OwnedRunFailed {
        exit_code: 1,
        output: PAGE.to_string(),
    };
    let message = error.to_string();

    assert!(
        message.contains("FAILED  any_readiness_resumes_when_a_child_is_ready"),
        "the error names the failing case:\n{message}"
    );
    assert!(
        message.contains("any_readiness_park: polled while parked"),
        "the error keeps the first mismatched value:\n{message}"
    );
    assert!(
        message.contains("1 passed; 1 failed"),
        "the error keeps the summary:\n{message}"
    );
}

#[test]
fn the_bindgen_failure_keeps_the_failed_case_and_its_message() {
    let error = WasmTestbedError::CaseFailures {
        failed: 1,
        output: PAGE.to_string(),
    };
    let message = error.to_string();

    assert!(
        message.contains("FAILED  any_readiness_resumes_when_a_child_is_ready"),
        "the error names the failing case:\n{message}"
    );
    assert!(
        message.contains("any_readiness_park: polled while parked"),
        "the error keeps the first mismatched value:\n{message}"
    );
}
