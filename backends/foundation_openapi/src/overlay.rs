//! Vendored patches merged into a vendor's `OpenAPI` spec.
//!
//! **WHY:** a vendor's spec can lack something a crate needs. Cloudflare's spec
//! has no R2 object-listing endpoint, and it types D1 query parameters as
//! strings. The fix is not a hand-written type in the generated crate (the next
//! regeneration would not know about it, and the type would drift from the
//! wire). The fix is a small reviewed file beside the spec that the generator
//! merges in, so every type still comes from the generator.
//!
//! **WHAT:** an overlay is a JSON document:
//!
//! ```json
//! {
//!   "description": "what this adds and why",
//!   "source": "the vendor page or observed behaviour it copies",
//!   "patch": { "paths": { "/some/path": { "get": { ... } } } }
//! }
//! ```
//!
//! `description` and `source` are required and must not be empty, so every
//! change to the vendor's spec says where it came from.
//!
//! **HOW:** `patch` is an RFC 7386 JSON Merge Patch: objects merge key by key,
//! `null` removes a key, and anything else (including an array) replaces the
//! value. [`apply_file`] applies one overlay; apply several in a fixed order.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// Why an overlay could not be applied.
#[derive(Debug)]
pub enum OverlayError {
    /// The overlay file could not be read.
    Unreadable {
        /// Path we tried.
        path: PathBuf,
        /// The underlying IO error.
        source: std::io::Error,
    },
    /// The overlay file is not JSON.
    Unparseable {
        /// Path we tried.
        path: PathBuf,
        /// The underlying parse error.
        source: serde_json::Error,
    },
    /// The overlay lacks a non-empty `description`, `source` or an object `patch`.
    Malformed {
        /// Path of the overlay.
        path: PathBuf,
        /// What is missing.
        missing: &'static str,
    },
}

impl std::fmt::Display for OverlayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable { path, source } => {
                write!(f, "cannot read overlay {}: {source}", path.display())
            }
            Self::Unparseable { path, source } => {
                write!(f, "overlay {} is not valid JSON: {source}", path.display())
            }
            Self::Malformed { path, missing } => write!(
                f,
                "overlay {} is malformed: it needs {missing}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for OverlayError {}

/// Read the overlay at `path` and merge its `patch` into `spec`.
///
/// # Errors
/// [`OverlayError`] for an unreadable or unparseable file, or one without a
/// `description`, a `source` and an object `patch`.
pub fn apply_file(spec: &mut Value, path: &Path) -> Result<(), OverlayError> {
    let raw = std::fs::read_to_string(path).map_err(|source| OverlayError::Unreadable {
        path: path.to_path_buf(),
        source,
    })?;
    let document: Value =
        serde_json::from_str(&raw).map_err(|source| OverlayError::Unparseable {
            path: path.to_path_buf(),
            source,
        })?;
    apply(spec, &document, path)
}

/// Merge the `patch` of an already parsed overlay `document` into `spec`. `path` names
/// the overlay in errors.
///
/// # Errors
/// [`OverlayError::Malformed`] when `description`, `source` or `patch` is
/// missing.
pub fn apply(spec: &mut Value, document: &Value, path: &Path) -> Result<(), OverlayError> {
    let text = |key: &str| {
        document
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|t| !t.trim().is_empty())
    };
    let malformed = |missing| OverlayError::Malformed {
        path: path.to_path_buf(),
        missing,
    };
    if !text("description") {
        return Err(malformed("a non-empty `description`"));
    }
    if !text("source") {
        return Err(malformed("a non-empty `source`"));
    }
    let merge = document
        .get("patch")
        .filter(|p| p.is_object())
        .ok_or_else(|| malformed("an object `patch`"))?;
    merge_patch(spec, merge);
    Ok(())
}

/// RFC 7386 JSON Merge Patch.
pub fn merge_patch(target: &mut Value, patch: &Value) {
    let Value::Object(patch_map) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = Value::Object(serde_json::Map::new());
    }
    let Value::Object(target_map) = target else {
        return;
    };
    for (key, value) in patch_map {
        if value.is_null() {
            target_map.remove(key);
        } else {
            merge_patch(target_map.entry(key.clone()).or_insert(Value::Null), value);
        }
    }
}
