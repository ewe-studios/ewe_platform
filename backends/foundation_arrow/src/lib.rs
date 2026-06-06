//! Arrow-based zero-copy serialization for any Rust type.
//!
//! Provides traits for converting Rust types to/from Arrow `RecordBatch`es,
//! enabling efficient columnar storage and IPC streaming. Works on native,
//! WASI, and WASM targets.
//!
//! # Traits
//!
//! - [`ToArrow`] — convert a type (or `Vec` of type) to an Arrow `RecordBatch`
//! - [`FromArrow`] — convert an Arrow `RecordBatch` back to a type
//! - [`ArrowSchema`] — generate an Arrow `Schema` for a type

pub mod ipc;
pub mod traits;

pub use arrow_array;
pub use arrow_buffer;
pub use arrow_data;
pub use arrow_ipc;
pub use arrow_schema;

pub use ipc::{decode_ipc, decode_ipc_batches, decode_ipc_schema, encode_ipc, encode_ipc_batches, IpcResult};
pub use traits::{ArrowSchema, FromArrow, ToArrow};
