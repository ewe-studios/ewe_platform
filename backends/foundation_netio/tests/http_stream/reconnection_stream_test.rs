use foundation_netio::netcap::connection::Endpoint;
use foundation_netio::netcap::ClientEndpoint;
use foundation_netio::http_stream::ReconnectionError;
use foundation_netio::http_stream::ReconnectingStream;
use foundation_netio::http_stream::ReconnectionStatus;
use foundation_core::panic_if_failed;
use foundation_core::retries::SameBackoffDecider;
use foundation_core::valtron::PoolGuard;
use std::{net::TcpListener, result::Result, thread};
use tracing;

use futures_lite::StreamExt;

#[test]
fn successfully_connects_on_first_try() {
    let _pool_guard: PoolGuard = foundation_core::valtron::initialize_pool(42, None);
    let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:3799"));
    let threader = thread::spawn(move || {
        let _ = listener.accept();
        tracing::debug!("Received client, ending");
    });

    let endpoint = foundation_netio::netcap::ClientEndpoint::Plain(panic_if_failed!(Endpoint::with_string(
        "http://127.0.0.1:3799"
    )));
    let mut stream = ReconnectingStream::new(
        2,
        endpoint,
        std::time::Duration::from_millis(500),
        SameBackoffDecider::new(std::time::Duration::from_millis(200)),
    );

    let collected: Option<Result<ReconnectionStatus, ReconnectionError>> = stream.next();
    dbg!(&collected);

    assert!(matches!(collected, Some(Ok(ReconnectionStatus::Ready(_)))));

    threader.join().expect("closed");
}

#[test]
fn fails_reconnection_after_max_retries() {
    let _pool_guard: PoolGuard = foundation_core::valtron::initialize_pool(42, None);
    let endpoint = foundation_netio::netcap::ClientEndpoint::Plain(panic_if_failed!(Endpoint::with_string(
        "http://127.0.0.1:8899"
    )));
    let stream = ReconnectingStream::new(
        2,
        endpoint,
        std::time::Duration::from_millis(50),
        SameBackoffDecider::new(std::time::Duration::from_millis(200)),
    );

    let collected: Vec<Result<ReconnectionStatus, ReconnectionError>> = stream
        .filter(|item| match item {
            Ok(inner) => match inner {
                ReconnectionStatus::Waiting(duration) => {
                    if duration == &std::time::Duration::from_millis(200) {
                        return true;
                    }
                    false
                }
                _ => true,
            },
            Err(_) => true,
        })
        .collect();

    dbg!(&collected);

    assert_eq!(
        collected[0..collected.len() - 1],
        vec![
            Ok(ReconnectionStatus::Waiting(std::time::Duration::from_millis(200))),
            Ok(ReconnectionStatus::NoMoreWaiting),
            Ok(ReconnectionStatus::Waiting(std::time::Duration::from_millis(200))),
            Ok(ReconnectionStatus::NoMoreWaiting),
        ]
    );

    assert!(matches!(collected[4], Err(ReconnectionError::Failed(_))));
}
