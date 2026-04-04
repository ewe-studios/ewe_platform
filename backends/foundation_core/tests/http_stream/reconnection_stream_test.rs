#![cfg(test)]

use foundation_core::{netcap::Endpoint, panic_if_failed, retries::SameBackoffDecider};
use std::{net::TcpListener, result::Result, thread};
use tracing;

use super::*;

#[test]
fn successfully_connects_on_first_try() {
    let listener = panic_if_failed!(TcpListener::bind("127.0.0.1:3799"));
    let threader = thread::spawn(move || {
        let _ = listener.accept();
        tracing::debug!("Received client, ending");
    });

    let endpoint = ClientEndpoint::Plain(panic_if_failed!(Endpoint::with_string(
        "http://127.0.0.1:3799"
    )));
    let mut stream = ReconnectingStream::new(
        2,
        endpoint,
        Duration::from_millis(500),
        SameBackoffDecider::new(Duration::from_millis(200)),
    );

    let collected: Option<Result<ReconnectionStatus, ReconnectionError>> = stream.next();
    dbg!(&collected);

    assert!(matches!(collected, Some(Ok(ReconnectionStatus::Ready(_)))));

    threader.join().expect("closed");
}

#[test]
fn fails_reconnection_after_max_retries() {
    let endpoint = ClientEndpoint::Plain(panic_if_failed!(Endpoint::with_string(
        "http://127.0.0.1:8899"
    )));
    let stream = ReconnectingStream::new(
        2,
        endpoint,
        Duration::from_millis(50),
        SameBackoffDecider::new(Duration::from_millis(200)),
    );

    let collected: Vec<Result<ReconnectionStatus, ReconnectionError>> = stream
        .filter(|item| match item {
            Ok(inner) => match inner {
                ReconnectionStatus::Waiting(duration) => {
                    if duration == &Duration::from_millis(200) {
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
            Ok(ReconnectionStatus::Waiting(Duration::from_millis(200))),
            Ok(ReconnectionStatus::NoMoreWaiting),
            Ok(ReconnectionStatus::Waiting(Duration::from_millis(200))),
            Ok(ReconnectionStatus::NoMoreWaiting),
        ]
    );

    assert!(matches!(collected[4], Err(ReconnectionError::Failed(_))));
}
