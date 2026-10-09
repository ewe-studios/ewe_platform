-- Migration 022: Add r2_key column for R2 blob offload (F23).
-- When non-NULL, the document content lives in R2 at this key
-- and the D1 `content` column holds a placeholder.
ALTER TABLE documents ADD COLUMN r2_key TEXT;
