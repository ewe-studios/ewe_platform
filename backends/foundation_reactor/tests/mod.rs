// Test modules for foundation_reactor.
mod poll_wake;
#[cfg(all(target_os = "linux", feature = "uring"))]
mod send_pool;
mod shutdown_signal;
