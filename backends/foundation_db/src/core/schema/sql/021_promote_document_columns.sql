-- Migration 021: promote searchable columns on documents (F06 Part 2).
-- Nullable; the JSON `content` blob remains the source of truth. Populated on
-- write via PromotableDocument; enables filter/sort without parsing blobs.
ALTER TABLE documents ADD COLUMN title TEXT;
ALTER TABLE documents ADD COLUMN summary TEXT;
ALTER TABLE documents ADD COLUMN record_type TEXT;

CREATE INDEX IF NOT EXISTS idx_documents_collection_type
    ON documents(collection_key, record_type);
