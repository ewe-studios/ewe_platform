//! Fetch state/outcome/result and error-collector tests (moved from `src/http/tasks/state.rs`).

use foundation_netio::http::{
    ErrorCollector, FetchOutcome, FetchPending, FetchResult, HttpRequestPending,
};

#[test]
fn test_fetch_pending_display() {
    // Test Connecting state
    let connecting: FetchPending<&str> = FetchPending::connecting("api.example.com");
    assert_eq!(format!("{connecting}"), "api.example.com: Connecting...");

    // Test AwaitingResponse state
    let awaiting: FetchPending<&str> = FetchPending::awaiting_response("api.example.com");
    assert_eq!(
        format!("{awaiting}"),
        "api.example.com: Awaiting response..."
    );

    // Test Failed state
    let failed = FetchPending::failed("api.example.com", "Connection refused".to_string());
    assert_eq!(
        format!("{failed}"),
        "api.example.com: FAILED - Connection refused"
    );

    // Test Completed state
    let completed: FetchPending<&str> = FetchPending::completed("api.example.com");
    assert_eq!(format!("{completed}"), "api.example.com: Completed");
}

#[test]
fn test_fetch_pending_constructors() {
    let connecting: FetchPending<&str> = FetchPending::connecting("test");
    assert!(matches!(
        connecting,
        FetchPending::Connecting { source: "test" }
    ));

    let awaiting: FetchPending<&str> = FetchPending::awaiting_response("test");
    assert!(matches!(
        awaiting,
        FetchPending::AwaitingResponse { source: "test" }
    ));

    let failed: FetchPending<&str, &str> = FetchPending::failed("test", "error");
    assert!(matches!(
        failed,
        FetchPending::Failed {
            source: "test",
            error: "error"
        }
    ));

    let completed: FetchPending<&str> = FetchPending::completed("test");
    assert!(matches!(
        completed,
        FetchPending::Completed { source: "test" }
    ));
}

#[test]
fn test_fetch_pending_from_http_request() {
    let pending_stream: FetchPending<&str> =
        FetchPending::from_http_request(HttpRequestPending::WaitingForStream, "test");
    assert!(matches!(pending_stream, FetchPending::Connecting { .. }));

    let pending_intro: FetchPending<&str> =
        FetchPending::from_http_request(HttpRequestPending::WaitingIntroAndHeaders, "test");
    assert!(matches!(
        pending_intro,
        FetchPending::AwaitingResponse { .. }
    ));
}

#[test]
fn test_fetch_pending_generic_source() {
    // Test with String source
    let string_source: FetchPending<String> = FetchPending::connecting("api".to_string());
    assert_eq!(format!("{string_source}"), "api: Connecting...");

    // Test with custom error type
    let custom_error: FetchPending<&str, &'static str> = FetchPending::failed("api", "err");
    assert_eq!(format!("{custom_error}"), "api: FAILED - err");
}

#[test]
fn test_fetch_outcome_basic() {
    let success: FetchOutcome<i32, &str> = FetchOutcome::success(42);
    assert_eq!(success.clone().ok(), Some(42));
    assert_eq!(success.clone().err(), None);

    let error: FetchOutcome<i32, &str> = FetchOutcome::error("failed");
    assert_eq!(error.clone().ok(), None);
    assert_eq!(error.clone().err(), Some("failed"));
}

#[test]
fn test_fetch_outcome_map() {
    let success: FetchOutcome<i32, &str> = FetchOutcome::success(42);
    let mapped = success.map(|x| x * 2);
    assert_eq!(mapped.clone().ok(), Some(84));

    let error: FetchOutcome<i32, &str> = FetchOutcome::error("failed");
    let mapped_error = error.map(|x| x * 2);
    assert_eq!(mapped_error.clone().err(), Some("failed"));
}

#[test]
fn test_fetch_result_constructors() {
    let success: FetchResult<&str, Vec<i32>, String> =
        FetchResult::success("api1", vec![1, 2, 3]);
    assert_eq!(success.source, "api1");
    assert!(matches!(success.outcome, FetchOutcome::Success(_)));
    assert_eq!(success.clone().ok(), Some(vec![1, 2, 3]));

    let error: FetchResult<&str, Vec<i32>, &str> =
        FetchResult::error("api2", "connection failed");
    assert_eq!(error.source, "api2");
    assert!(matches!(error.outcome, FetchOutcome::Error(_)));
    assert_eq!(error.clone().err(), Some("connection failed"));
}

#[test]
fn test_fetch_result_new() {
    let result: FetchResult<&str, i32, String> =
        FetchResult::new("api", FetchOutcome::<i32, String>::success(100));
    assert_eq!(result.source, "api");
    assert_eq!(result.clone().ok(), Some(100));
}

#[test]
fn test_fetch_result_map() {
    let result: FetchResult<&str, Vec<i32>, String> =
        FetchResult::success("api", vec![1, 2, 3]);
    let mapped = result.map(|v: Vec<i32>| v.len());
    assert_eq!(mapped.source, "api");
    assert_eq!(mapped.ok(), Some(3));

    let error_result: FetchResult<&str, Vec<i32>, &str> = FetchResult::error("api", "failed");
    let mapped_error = error_result.map(|v: Vec<i32>| v.len());
    assert_eq!(mapped_error.err(), Some("failed"));
}

#[test]
fn test_fetch_result_display() {
    let success: FetchResult<&str, i32, &str> = FetchResult::success("api", 42);
    assert_eq!(format!("{success}"), "api: Success");

    let error: FetchResult<&str, i32, &str> = FetchResult::error("api", "timeout");
    assert_eq!(format!("{error}"), "api: Error - timeout");
}

#[test]
fn test_error_collector_basic() {
    let collector: ErrorCollector<String, String> = ErrorCollector::new();
    assert!(!collector.has_errors());
    assert_eq!(collector.count(), 0);

    collector.record("api1".to_string(), "error1".to_string());
    assert!(collector.has_errors());
    assert_eq!(collector.count(), 1);

    collector.record("api2".to_string(), "error2".to_string());
    assert_eq!(collector.count(), 2);
}

#[test]
fn test_error_collector_collect() {
    let collector: ErrorCollector<String, String> = ErrorCollector::new();
    collector.record("api1".to_string(), "error1".to_string());
    collector.record("api2".to_string(), "error2".to_string());

    let errors = collector.collect();
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0], ("api1".to_string(), "error1".to_string()));
    assert_eq!(errors[1], ("api2".to_string(), "error2".to_string()));
}

#[test]
fn test_error_collector_clone() {
    let collector = ErrorCollector::new();
    let clone = collector.clone();

    clone.record("api", "error");
    assert_eq!(collector.count(), 1);
    assert_eq!(clone.count(), 1);
}
