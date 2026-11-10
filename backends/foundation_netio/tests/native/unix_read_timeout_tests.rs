//! Regression test for issue #420: `Connection::connect_unix` must apply the
//! configured read timeout, not a hard-coded value.
//!
//! HOW: bind a real `UnixListener` at a temporary path, connect with a custom
//! timeout, and read the timeout back from the socket. No sleep and no timer:
//! the assertion reads the kernel socket option.

#![cfg(unix)]

use std::time::Duration;

use foundation_netio::native::connection::Connection;
use foundation_netio::shared::client::ClientConfig;

/// The default matches the old hard-coded 5 seconds.
#[test]
fn client_config_default_unix_read_timeout_is_five_seconds() {
    assert_eq!(
        ClientConfig::default().unix_read_timeout,
        Duration::from_secs(5)
    );
}

/// A configured timeout reaches the connected Unix socket.
#[test]
fn connect_unix_applies_the_configured_read_timeout() {
    let path = std::env::temp_dir().join(format!(
        "foundation-netio-unix-timeout-{}.sock",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let listener = std::os::unix::net::UnixListener::bind(&path).expect("bind Unix listener");

    let configured = Duration::from_millis(1500);
    let config = ClientConfig::default().with_unix_read_timeout(configured);

    let connection = Connection::connect_unix(&path, config.unix_read_timeout, &ctx())
        .expect("connect to the Unix socket");
    let observed = connection
        .read_timeout()
        .expect("query the socket read timeout");

    assert_eq!(
        observed,
        Some(configured),
        "connect_unix must set the configured read timeout on the socket"
    );

    drop(connection);
    drop(listener);
    let _ = std::fs::remove_file(&path);
}

fn ctx() -> foundation_core::valtron::Ctx {
    foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
        foundation_core::valtron::MonotonicClock::new(),
    ))
}
