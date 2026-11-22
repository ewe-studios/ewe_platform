//! Media types for a file name, from one compile-time table.
//!
//! WHY: every crate that serves a file needs the same answer, and a hand-written
//! match arm missed `.mjs` once (issue #578).
//!
//! WHAT: [`from_extension`] and [`from_path`], backed by `data/mime.json`, which
//! `generate_mime.py` builds from mime-db 1.54.0.
//!
//! HOW: `include_json!` expands the file into a `phf::Map` at build time. No
//! file is opened and no JavaScript Object Notation (JSON) is parsed at run
//! time.
//!
//! The lookup returns the bare media type, never a parameter (rule R6). A
//! caller that needs a `charset` parameter appends it.

use std::path::Path;

/// Every media type, by extension. Built at compile time (rule R2).
///
/// The data file is `data/mime.json` in this crate. A caller with a known
/// lower-case extension can use `TYPES.get(ext).copied()` directly, with no
/// allocation.
pub static TYPES: ::foundation_nostd::phf::Map<&'static str, &'static str> =
    ::foundation_macros::include_json!("data/mime.json");

/// The fallback media type for an unknown name.
///
/// This constant is the caller's choice, not a module decision (rule R3). The
/// lookup returns `None` for an unknown name.
pub const OCTET_STREAM: &str = "application/octet-stream";

/// The media type for `ext`, without the dot and without case.
///
/// The function lower-cases only text that holds an upper-case letter. It
/// returns `None` for an unknown name and never panics (invariant I3).
///
/// ```ignore
/// assert_eq!(from_extension(".MJS"), Some("text/javascript"));
/// assert_eq!(from_extension("nope"), None);
/// ```
#[must_use = "the caller picks the fallback; an unknown name returns None"]
pub fn from_extension(ext: &str) -> Option<&'static str> {
    let ext = ext.trim_start_matches('.');
    if ext.bytes().any(|byte| byte.is_ascii_uppercase()) {
        TYPES.get(ext.to_ascii_lowercase().as_str()).copied()
    } else {
        TYPES.get(ext).copied()
    }
}

/// The media type for the last extension of `path`.
///
/// A file name with no extension, or a dot file such as `.bashrc`, returns
/// `None`. The function reads the last extension only (rule R4).
///
/// ```ignore
/// assert_eq!(from_path("/a/file.mjs"), Some("text/javascript"));
/// assert_eq!(from_path("/a/file"), None);
/// assert_eq!(from_path("/a/.bashrc"), None);
/// ```
#[must_use = "the caller picks the fallback; an unknown name returns None"]
pub fn from_path(path: impl AsRef<Path>) -> Option<&'static str> {
    path.as_ref().extension()?.to_str().and_then(from_extension)
}
