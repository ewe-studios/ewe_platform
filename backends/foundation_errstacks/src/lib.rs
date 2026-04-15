//! # `foundation_errstacks`
//!
//! **WHY:** The `ewe_platform` backends need a minimal, ergonomic,
//! context-aware error-handling story without dragging in `anyhow`/`eyre`
//! or a global hook system. This crate extracts the core value proposition
//! of [`error-stack`] — a stack of contexts and attachments — while
//! remaining small, `derive_more`-friendly, and `no_std + alloc` capable.
//!
//! **WHAT:** Provides [`ErrorTrace<C>`], a typed error trace carrying a
//! stack of [`Frame`]s where `C` is the "current context" (the error type
//! through which the failure is currently interpreted). Callers move
//! between contexts with `change_context`, enrich traces with `attach`
//! (printable) and `attach_opaque` (programmatic), and later inspect
//! frames via iteration or downcasting.
//!
//! **HOW:** Frames are stored in a heap-allocated `Vec<Frame>` boxed
//! behind an indirection to keep `ErrorTrace<C>` a thin handle. The
//! current-context type parameter is phantom-typed (`PhantomData<fn() ->
//! *const C>`) so that `ErrorTrace<C>` remains `Send + Sync` and
//! covariant-free while still enforcing context awareness at compile
//! time.
//!
//! Feature matrix (see `specifications/08-foundation-errstacks/requirements.md` §3.5):
//!
//! | Feature      | Default | Requires   | Effect                                        |
//! |--------------|---------|------------|-----------------------------------------------|
//! | `alloc`      | yes     | —          | Baseline; `alloc::{Box, String, Vec}`.        |
//! | `std`        | yes     | `alloc`    | Enables `std`-only niceties + `Backtrace`.    |
//! | `backtrace`  | no      | `std`      | Turns on `std::backtrace::Backtrace` capture. |
//! | `serde`      | no      | `alloc`    | `Serialize` impls for the structured view.    |
//! | `async`      | no      | `std`      | Future extension traits.                      |
//! | `slack`      | no      | `serde`    | Slack block JSON helpers.                     |
//!
//! `core::error::Error` (stable since Rust 1.81) is the primary error
//! bound throughout the crate, so the same code compiles with or without
//! `std`.
//!
//! [`error-stack`]: https://github.com/hashintel/hash/tree/main/libs/error-stack

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]

extern crate alloc;

pub mod error_trace;
pub mod frame;
pub mod result_ext;

pub use error_trace::ErrorTrace;
#[cfg(feature = "to_structured")]
pub use error_trace::{StructuredErrorTrace, StructuredFrame};
pub use frame::{AttachmentKind, Frame, FrameIter, FrameKind};
pub use result_ext::{ErrorTraceResultExt, IntoErrorTrace, PlainResultExt};
