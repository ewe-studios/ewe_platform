//! Icons SSRF-guard tests (spec-57, F008 Stage 1).
//!
//! The security-critical part of the icon proxy is refusing to fetch internal
//! addresses — those are rejected before any network call, so these need no
//! network. (The happy-path fetch is covered by the transport integration tests.)

use foundation_core::valtron::{collect_one, execute, from_future, valtron_test};
use foundation_keychain::core::api::icons;
use foundation_keychain::AppError;

fn drive<T, F>(future: F) -> T
where
    T: Send + 'static,
    F: std::future::Future<Output = T> + Send + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None).expect("valtron execute");
    collect_one(stream).expect("future produced a result")
}

fn fetch(domain: &'static str) -> foundation_keychain::core::error::AppResult<icons::IconData> {
    drive(async move { icons::fetch_icon(domain).await })
}

#[valtron_test]
fn rejects_ip_literals_and_internal_hosts() {
    // Bare IP literals (incl. private ranges) never reach the network.
    assert!(
        matches!(fetch("127.0.0.1"), Err(trace) if matches!(trace.current_context(), AppError::BadRequest(_)))
    );
    assert!(
        matches!(fetch("192.168.1.1"), Err(trace) if matches!(trace.current_context(), AppError::BadRequest(_)))
    );
    assert!(
        matches!(fetch("10.0.0.1"), Err(trace) if matches!(trace.current_context(), AppError::BadRequest(_)))
    );
    assert!(
        matches!(fetch("169.254.1.1"), Err(trace) if matches!(trace.current_context(), AppError::BadRequest(_)))
    );

    // `localhost` has no dot → not a plausible registrable domain.
    assert!(
        matches!(fetch("localhost"), Err(trace) if matches!(trace.current_context(), AppError::BadRequest(_)))
    );

    // `.local` / `.localhost` suffixes are blocked by the SSRF guard.
    assert!(
        matches!(fetch("printer.local"), Err(trace) if matches!(trace.current_context(), AppError::Forbidden))
    );
    assert!(
        matches!(fetch("app.localhost"), Err(trace) if matches!(trace.current_context(), AppError::Forbidden))
    );
}

#[valtron_test]
fn rejects_malformed_domains() {
    assert!(
        matches!(fetch(""), Err(trace) if matches!(trace.current_context(), AppError::BadRequest(_)))
    );
    assert!(matches!(
        fetch("has/slash.com"),
        Err(trace) if matches!(trace.current_context(), AppError::BadRequest(_))
    ));
    assert!(matches!(
        fetch("user@evil.com"),
        Err(trace) if matches!(trace.current_context(), AppError::BadRequest(_))
    ));
    assert!(matches!(
        fetch("evil.com:8080"),
        Err(trace) if matches!(trace.current_context(), AppError::BadRequest(_))
    ));
    assert!(
        matches!(fetch("no-dot-host"), Err(trace) if matches!(trace.current_context(), AppError::BadRequest(_)))
    );
    assert!(matches!(
        fetch("double..dot.com"),
        Err(trace) if matches!(trace.current_context(), AppError::BadRequest(_))
    ));
}
