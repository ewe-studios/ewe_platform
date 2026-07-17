# Decision 07: Blob Storage — foundation_db BlobStore

## Problem

Cipher attachments and Send files need object storage. Options:

1. **foundation_db BlobStore** — `put_blob`, `get_blob`, `delete_blob`, `blob_exists` — already implemented for all backends
2. **Custom trait** — unnecessary duplication

## Analysis

`foundation_db::BlobStore` is already implemented for every backend:
- **Cloudflare**: `R2Wasm` (via wasm-bindgen R2 JS API)
- **Native**: `Turso` (embedded SQLite blob), `Libsql` (embedded SQLite blob), `JsonFile` (filesystem)
- **Memory**: `MemoryStorage` (for testing)

The API is simple: `put_blob(key, data)`, `get_blob(key)`, `delete_blob(key)`, `blob_exists(key)`. Exactly what OrangeVault needs for attachments and send files.

## Decision: foundation_db BlobStore

Cloudflare backend uses `StorageBackend::R2Wasm`. Native backend uses `StorageBackend::JsonFile` (filesystem) or `StorageBackend::Turso` (embedded blob table). No custom blob trait.

## Consequences

- No additional dependencies
- File paths for native: `$KEYCHAIN_DATA/blobs/{cipher_uuid}/{attachment_id}` or `sends/{send_uuid}/{file_id}`
- Single-instance only for native (no shared blob storage across instances)
- S3 support would require a new `foundation_db` backend — deferred until multi-instance is needed
