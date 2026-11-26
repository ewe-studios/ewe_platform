//! The control-plane tests.
//!
//! These run the real slave client, codec and router in the same process with
//! no socket. They need the `testing` feature.
#![allow(
    clippy::assert_is_empty,
    clippy::explicit_counter_loop,
    clippy::items_after_statements,
    clippy::needless_pass_by_value,
    reason = "the tests read clearer with these forms"
)]

pub mod support;

mod create;
mod drop;
mod exec;
mod misc;
mod properties;
mod restore;
mod rpc;
mod stop;

use foundation_core::valtron::Stream;
use foundation_sandbox::shared::control::SandboxHandle;
use foundation_sandbox::shared::driver::CreateProgress;
use foundation_sandbox::shared::error::SandboxResult;
use foundation_sandbox::shared::handles::CreateStream;

/// Reads a create stream to its one result.
///
/// # Panics
/// When the stream ends without a result.
pub fn drain_create(
    stream: &mut CreateStream,
) -> (Vec<CreateProgress>, SandboxResult<SandboxHandle>) {
    let mut progress = Vec::new();
    loop {
        match stream.next() {
            Some(Stream::Pending(value)) => progress.push(value),
            Some(Stream::Next(result)) => return (progress, result),
            Some(
                Stream::Ignore
                | Stream::Init
                | Stream::Wait
                | Stream::Delayed(_)
                | Stream::Spread(_),
            ) => {}
            None => panic!("the create stream ended without a result"),
        }
    }
}
