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
//! # Feature Matrix
//!
//! | Feature      | Default | Requires   | Effect                                        |
//! |--------------|---------|------------|-----------------------------------------------|
//! | `alloc`      | yes     | —          | Baseline; `alloc::{Box, String, Vec}`.        |
//! | `std`        | yes     | `alloc`    | Enables `std`-only niceties + `Backtrace`.    |
//! | `backtrace`  | no      | `std`      | Turns on `std::backtrace::Backtrace` capture. |
//! | `serde`      | no      | `alloc`    | `Serialize` impls for the structured view.    |
//! | `to_structured` | no   | `serde`    | `to_structured()` method for JSON output.     |
//! | `async`      | no      | `std`      | Future extension traits.                      |
//! | `slack`      | no      | `to_structured` | Slack block JSON helpers.              |
//! | `short_display` | no   | —          | Display shows only current context (not full tree). |
//!
//! # Quick Start
//!
//! ```rust
//! # #[cfg(feature = "std")] {
//! use foundation_errstacks::{ErrorTrace, PlainResultExt};
//! use derive_more::{Display, Error, From};
//!
//! // Enum is the preferred default — each variant carries its own context.
//! // Structs work too; use whichever fits your error domain.
//! #[derive(Debug, Display, Error, From)]
//! enum DbError {
//!     #[display("database connection failed")]
//!     Connect,
//!     #[display("query failed on table '{table}': {reason}")]
//!     Query { table: String, reason: String },
//! }
//!
//! fn connect() -> Result<(), ErrorTrace<DbError>> {
//!     Err(DbError::Connect).attach("host=db.example.com")
//! }
//! # }
//! ```
//!
//! # Integrating with `derive_more`
//!
//! This crate is designed to work seamlessly with [`derive_more`] v2 for
//! ergonomic error type definitions. Here's a complete example:
//!
//! ## Basic Setup
//!
//! Add these dependencies to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! foundation_errstacks = "0.0.1"
//! derive_more = { version = "2", features = ["display", "error", "from"] }
//! ```
//!
//! ## Defining Error Types with `derive_more`
//!
//! ```rust
//! # #[cfg(feature = "std")] {
//! use derive_more::{Display, Error};
//! use foundation_errstacks::{ErrorTrace, PlainResultExt, ErrorTraceResultExt};
//!
//! // Simple error with Display and Error derived
//! #[derive(Debug, Display, Error)]
//! #[display("file not found")]
//! struct FileNotFound;
//!
//! // Enum is the preferred default — each variant carries its own context.
//! // Structs work too; use whichever fits your error domain.
//! #[derive(Debug, Display, Error, From)]
//! enum ApiError {
//!     #[display("file not found")]
//!     FileNotFound,
//!     #[display("parse error at line {line}, column {col}: {message}")]
//!     ParseError { line: u32, col: u32, message: &'static str },
//! }
//!
//! // Using ErrorTrace with derived error types
//! fn read_config() -> Result<(), ErrorTrace<ApiError>> {
//!     Err(ApiError::FileNotFound).attach("path=/etc/config.toml")
//! }
//!
//! fn process_file() -> Result<(), ErrorTrace<ApiError>> {
//!     read_config()
//!         .map_err(|trace| trace.change_context(ApiError::ParseError { line: 1, col: 5, message: "expected '='" }))
//! }
//! # }
//! ```
//!
//! ## Chaining Errors Across Modules
//!
//! ```rust
//! # #[cfg(feature = "std")] {
//! use derive_more::{Display, Error};
//! use foundation_errstacks::{ErrorTrace, PlainResultExt, ErrorTraceResultExt};
//!
//! #[derive(Debug, Display, Error, From)]
//! enum AppError {
//!     #[display("lower level error")]
//!     Lower,
//!     #[display("higher level context")]
//!     Higher,
//! }
//!
//! fn inner() -> Result<(), ErrorTrace<AppError>> {
//!     Err(AppError::Lower).attach("debug info")
//! }
//!
//! fn outer() -> Result<(), ErrorTrace<AppError>> {
//!     ErrorTraceResultExt::change_context(inner(), AppError::Higher)
//! }
//!
//! // The resulting trace contains both contexts:
//! // - LowerError (original)
//! // - "debug info" attachment
//! // - HigherError (transformed context)
//! # }
//! ```
//!
//! # Feature-Gated Usage
//!
//! ## `no_std` Environments
//!
//! ```toml
//! [dependencies]
//! foundation_errstacks = { version = "0.0.1", default-features = false, features = ["alloc"] }
//! ```
//!
//! ## With Serialization
//!
//! ```toml
//! [dependencies]
//! foundation_errstacks = { version = "0.0.1", features = ["serde", "to_structured"] }
//! ```
//!
//! ## With Slack Integration
//!
//! ```toml
//! [dependencies]
//! foundation_errstacks = { version = "0.0.1", features = ["slack"] }
//! ```
//!
//! ```rust
//! # #[cfg(all(feature = "to_structured", feature = "slack"))] {
//! # use derive_more::{Display, Error};
//! # use foundation_errstacks::ErrorTrace;
//! # #[derive(Debug, Display, Error)]
//! # enum AlertError {
//! #     #[display("alert!")]
//! #     Critical,
//! # }
//! let trace = ErrorTrace::new(AlertError::Critical).attach("severity=critical");
//! let structured = trace.to_structured();
//! let slack_json = structured.to_slack_json().unwrap();
//! // Send `slack_json` to your Slack webhook
//! # }
//! ```
//!
//! # Core API Overview
//!
//! - [`ErrorTrace<C>`] — The main error trace type
//! - [`PlainResultExt`] — Extension trait for `Result<T, E>` (plain errors)
//! - [`ErrorTraceResultExt`] — Extension trait for `Result<T, ErrorTrace<C>>`
//! - [`Frame`], [`FrameIter`] — Frame inspection and iteration
//!
//! [`error-stack`]: https://github.com/hashintel/hash/tree/main/libs/error-stack
//! [`derive_more`]: https://docs.rs/derive_more/2
//! [`ErrorTrace<C>`]: crate::ErrorTrace
//! [`PlainResultExt`]: crate::PlainResultExt
//! [`ErrorTraceResultExt`]: crate::ErrorTraceResultExt
//! [`Frame`]: crate::Frame
//! [`FrameIter`]: crate::FrameIter

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]

extern crate alloc;

pub mod error_trace;
pub mod frame;
pub mod result_ext;

pub use error_trace::ErrorTrace;
#[cfg(feature = "slack")]
pub use error_trace::{SlackBlock, SlackBlocks, SlackTextObject};
#[cfg(feature = "to_structured")]
pub use error_trace::{StructuredErrorTrace, StructuredFrame};
pub use frame::{AttachmentKind, Frame, FrameIter, FrameKind};
pub use result_ext::{ErrorTraceResultExt, IntoErrorTrace, PlainResultExt};
