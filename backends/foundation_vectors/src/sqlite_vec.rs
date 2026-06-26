//! sqlite-vec extension loader for the SQLite VectorStore backend (F29).
//!
//! The prebuilt `vec0.so` is embedded in the binary (or loaded from
//! `vendor/vec0.so` at runtime). On init the extension is written to a temp
//! file and loaded into libSQL via `load_extension`. After loading the
//! `vec0` virtual table type is available for KNN queries.
