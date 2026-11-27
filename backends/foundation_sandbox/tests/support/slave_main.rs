//! The slave process of the tests.
//!
//! A libtest test cannot serve as the slave's process. The harness starts a
//! thread before the test runs, so the signal mask of the process's main thread
//! stays clear. `SIGCHLD` can go to that thread and the `signalfd` misses it,
//! and then the reaper never collects a child. This program blocks `SIGCHLD`
//! as its first statement, before any thread starts, as the slave needs.
//!
//! `tests/slave/support.rs` starts it with `CARGO_BIN_EXE_slave_test_driver`.

#[cfg(target_os = "linux")]
fn main() {
    use foundation_sandbox::native::slave::{block_sigchld, run, ListenAddr, SlaveArgs};

    block_sigchld().expect("the slave blocks SIGCHLD");
    let socket = std::env::var("EWE_SLAVE_SOCKET").expect("EWE_SLAVE_SOCKET is set");
    let plain_root = std::env::var("EWE_SLAVE_PLAIN_ROOT").unwrap_or_else(|_| "/".to_string());
    let threads: usize = std::env::var("EWE_SLAVE_THREADS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8);
    let socket_mode = std::env::var("EWE_SLAVE_SOCKET_MODE")
        .ok()
        .and_then(|value| u32::from_str_radix(value.trim_start_matches("0o"), 8).ok())
        .unwrap_or(0o600);
    std::mem::forget(foundation_core::valtron::initialize_pool(1, Some(threads)));
    let control = std::env::var("EWE_SLAVE_CONTROL").is_ok_and(|value| value == "1");
    let args = SlaveArgs {
        listen: ListenAddr::Unix {
            path: socket.into(),
        },
        control,
        plain_root: plain_root.into(),
        socket_mode,
    };
    let code = run(foundation_context::Ctx::host(), args);
    std::process::exit(code);
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("the slave test driver needs Linux");
    std::process::exit(1);
}
