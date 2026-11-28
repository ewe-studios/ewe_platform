//! The valtron layer for Durable Streams.
//!
//! The protocol core stays free of valtron. This module wraps it with
//! `Stream`, `Stream::Wait` and `#[task]`. The public path is
//! `foundation_core::durable::valtron`.

mod client;
mod server;

pub use client::{ClientReadProgress, ClientReader};
pub use server::{
    producer_task, DurableStreamReader, DurableStreamWriter, ProducerProgress, ReaderProgress,
};
