//! Probe whether io_uring is available in this environment (spec 70 feature 78 E1).
//!
//! WHY: the `foundation_iogate` io_uring tests fail when `EWE_REQUIRE_IO_URING=1`
//! and io_uring is missing. CI sets that variable, so the job must record first
//! whether the kernel and the container's seccomp profile allow `io_uring_setup`.
//! A failure then says "the environment blocks io_uring", not "the code is
//! broken".
//!
//! WHAT: start the shared reactor with `ServerIo::Completion`, which calls
//! `io_uring_setup` through the crate's own detection, and print the result.
//!
//! HOW: `cargo test -p foundation_iogate --test io_uring_probe -- --nocapture`
//! runs in its own process, so it does not change the reactor of the package
//! tests. The test always passes; the CI step reads the printed line.

#![cfg(target_os = "linux")]

use foundation_iogate::ServerIo;

#[test]
fn probe_io_uring() {
    match foundation_iogate::init_reactor_for(ServerIo::Completion) {
        Ok(()) => println!("io_uring probe: available (completion reactor started)"),
        Err(e) => println!("io_uring probe: unavailable: {e}"),
    }
}
