-- Migration 020: Create documents table for DocumentStore
CREATE TABLE IF NOT EXISTS documents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    collection_key TEXT NOT NULL,
    doc_id TEXT NOT NULL,
    content TEXT NOT NULL,
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_documents_collection_key ON documents(collection_key);
CREATE INDEX IF NOT EXISTS idx_documents_collection_created ON documents(collection_key, created_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_documents_collection_doc_id ON documents(collection_key, doc_id);
