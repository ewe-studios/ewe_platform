//! Integration tests for `Ssh2Backend` — Docker-free paths.
//!
//! **WHY:** Docker-backed tests live in `foundation_deployment_platform` (the
//! orchestration layer that consumes SSH).  This file covers only the paths that
//! need no container — connection-refused is enough to exercise the connect-failure
//! branch.
//!
//! **WHAT:** Connection to a closed port must return a connect error, not hang or
//! panic.

use std::sync::Arc;
use std::time::Duration;

use foundation_sshkit::Backend;
use foundation_sshkit::{Command, ConnectionPool, Host, Ssh2Backend};

#[test]
fn test_execute_connection_refused_returns_error() {
    let pool = Arc::new(ConnectionPool::new(Duration::from_secs(2)));
    let backend = Ssh2Backend::new(pool);
    let host = Host::parse("root@127.0.0.1:1");

    let result = backend.execute(&host, &Command::new("true"));

    assert!(result.is_err(), "connecting to a closed port must fail");
    let err = result.unwrap_err();
    assert!(
        err.contains("connect"),
        "error should originate from the TCP connect, got: {err:?}"
    );
}
