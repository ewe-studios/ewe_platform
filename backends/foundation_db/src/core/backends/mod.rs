//! Core backends — shared between native and wasm.

pub mod async_sql_document_store;
pub mod memory;
pub mod memory_json;
pub mod memory_document_store;
pub mod sql_document_store;
pub mod sql_vector_store;
pub mod turbopuffer_vector_store;

// Re-exports
pub use async_sql_document_store::AsyncSqlDocumentStore;
pub use memory::*;
pub use memory_json::*;
pub use memory_document_store::MemoryDocumentStore;
pub use sql_document_store::SqlDocumentStore;
pub use sql_vector_store::SqlVectorStore;
pub use turbopuffer_vector_store::TurboPufferVectorStore;
