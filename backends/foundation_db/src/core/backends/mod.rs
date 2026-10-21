//! Core backends — shared between native and wasm.

pub mod d1r2_document_store;
pub mod memory;
pub mod memory_json;
pub mod memory_document_store;
pub mod sql_document_store;

// Re-exports
pub use d1r2_document_store::D1R2DocumentStore;
pub use memory::*;
pub use memory_json::*;
pub use memory_document_store::MemoryDocumentStore;
pub use sql_document_store::SqlDocumentStore;
