//! WHY: Effects produce `DomOp`s one at a time during `stabilize()`, but shipping
//! one message per op would be wasteful. The instruction layer batches them and
//! flushes once per cycle through the configured protocol (decision 030).
//!
//! WHAT: Re-exports [`InstructionReceiver`].
//!
//! HOW: See [`receiver`].

mod receiver;

pub use receiver::InstructionReceiver;
