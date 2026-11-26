// The sub-agent tool holds a `TurnStream`, which is `Send` only in the
// multi-threaded build. The rest of the agentic layer works on one thread.
#[cfg(feature = "multi")]
pub mod agent;
pub mod files;
pub mod memory;
pub mod search;
pub mod shed;
