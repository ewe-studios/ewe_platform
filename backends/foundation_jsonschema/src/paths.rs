//! JSON path tracking for validation error reporting.
//!
//! WHY: Validation errors need to report exactly where in the JSON instance
//! and schema document the failure occurred. Paths are formatted as JSON
//! Pointer strings (RFC 6901).
//!
//! WHAT: `Location` for materialized paths, `LazyLocation` for zero-allocation
//! path tracking during the happy path (valid instance).
//!
//! HOW: `Location` stores segments as a `Vec`. `LazyLocation` uses a linked-list
//! approach that defers string allocation until `materialize()` is called.

use alloc::{string::String, vec::Vec};
use core::fmt;

/// A segment in a JSON path — either a property name or array index.
///
/// WHY: JSON Pointer paths consist of either property names (objects) or
/// indices (arrays). This enum captures both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocationSegment {
    /// A property name in a JSON object.
    Property(String),
    /// An index in a JSON array.
    Index(usize),
}

impl fmt::Display for LocationSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Property(name) => write!(f, "{name}"),
            Self::Index(idx) => write!(f, "{idx}"),
        }
    }
}

/// A materialized JSON Pointer path (e.g., "/foo/bar/0").
///
/// WHY: Error reporting and structured output need full path strings.
/// This type owns the path segments for use after validation completes.
///
/// WHAT: An ordered list of `LocationSegment`s that forms a complete path.
///
/// HOW: Segments are appended during validation traversal. `as_json_pointer()`
/// produces the RFC 6901 string with `~0`/`~1` escaping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    segments: Vec<LocationSegment>,
}

impl Location {
    /// Create an empty location (root of the JSON document).
    #[must_use]
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    /// Push a property name segment.
    pub fn push_property(&mut self, name: impl Into<String>) {
        self.segments.push(LocationSegment::Property(name.into()));
    }

    /// Push an array index segment.
    pub fn push_index(&mut self, idx: usize) {
        self.segments.push(LocationSegment::Index(idx));
    }

    /// Remove the last segment. Returns `true` if a segment was removed.
    pub fn pop(&mut self) -> bool {
        self.segments.pop().is_some()
    }

    /// Return the segments as a slice.
    #[must_use]
    pub fn segments(&self) -> &[LocationSegment] {
        &self.segments
    }

    /// Format as an RFC 6901 JSON Pointer string.
    ///
    /// WHY: JSON Pointer is the standard path format used by JSON Schema
    /// for referencing locations within a document.
    ///
    /// WHAT: Returns a string like "/foo/bar/0".
    ///
    /// HOW: Joins segments with "/" and escapes "~" as "~0" and "/" as "~1"
    /// per RFC 6901 Section 3.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn as_json_pointer(&self) -> String {
        if self.segments.is_empty() {
            return String::new();
        }
        let mut result = String::new();
        for seg in &self.segments {
            result.push('/');
            match seg {
                LocationSegment::Property(name) => {
                    // RFC 6901 escaping: ~ → ~0, / → ~1
                    let chars = name.chars();
                    for c in chars {
                        match c {
                            '~' => result.push_str("~0"),
                            '/' => result.push_str("~1"),
                            _ => result.push(c),
                        }
                    }
                }
                LocationSegment::Index(idx) => {
                    let mut buf = itoa::Buffer::new();
                    let s = buf.format(*idx);
                    result.push_str(s);
                }
            }
        }
        result
    }
}

impl Default for Location {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pointer = self.as_json_pointer();
        if pointer.is_empty() {
            write!(f, "/")
        } else {
            write!(f, "{pointer}")
        }
    }
}

/// Lazy path construction — defers allocation until materialized.
///
/// WHY: During validation of valid instances, we never need path strings.
/// `LazyLocation` avoids allocating path segments until an error requires them.
/// This keeps the happy path (valid instance) allocation-free.
///
/// WHAT: A singly-linked stack of segments where each node holds a reference
/// to its parent. The path is built by traversing from leaf to root when
/// materialized.
///
/// HOW: `push_property()` and `push_index()` create new stack frames with
/// a reference to the current frame. `materialize()` walks the chain and
/// builds a `Location` string.
#[derive(Debug, Clone)]
pub struct LazyLocation<'a> {
    parent: Option<&'a LazyLocation<'a>>,
    segment: Option<LocationSegment>,
}

impl<'a> LazyLocation<'a> {
    /// Create a root lazy location (empty path).
    #[must_use]
    pub fn new() -> Self {
        Self {
            parent: None,
            segment: None,
        }
    }

    /// Push a property name, creating a new lazy location frame.
    ///
    /// WHY: Entering an object property during validation.
    ///
    /// WHAT: Returns a new `LazyLocation` that references `self` as parent.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn push_property(&'a self, name: &str) -> LazyLocation<'a> {
        LazyLocation {
            parent: Some(self),
            segment: Some(LocationSegment::Property(name.into())),
        }
    }

    /// Push an array index, creating a new lazy location frame.
    ///
    /// WHY: Entering an array element during validation.
    ///
    /// WHAT: Returns a new `LazyLocation` that references `self` as parent.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn push_index(&'a self, idx: usize) -> LazyLocation<'a> {
        LazyLocation {
            parent: Some(self),
            segment: Some(LocationSegment::Index(idx)),
        }
    }

    /// Materialize the lazy path into an owned `Location`.
    ///
    /// WHY: When an error occurs, we need the full path string for reporting.
    ///
    /// WHAT: Walks the parent chain from leaf to root, collecting segments,
    /// then builds a `Location` with them in order.
    ///
    /// HOW: Collects segments in reverse (leaf → root), reverses them,
    /// then constructs a `Location` with those segments.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn materialize(&self) -> Location {
        let mut segments = Vec::new();
        let mut current = self;

        // Collect from leaf to root
        loop {
            if let Some(ref seg) = current.segment {
                segments.push(seg.clone());
            }
            match current.parent {
                Some(parent) => current = parent,
                None => break,
            }
        }

        segments.reverse();

        Location { segments }
    }
}

impl Default for LazyLocation<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LazyLocation<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let location = self.materialize();
        write!(f, "{location}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_location_empty() {
        let loc = Location::new();
        assert!(loc.as_json_pointer().is_empty());
    }

    #[test]
    fn test_location_push_property() {
        let mut loc = Location::new();
        loc.push_property("foo");
        loc.push_property("bar");
        assert_eq!(loc.as_json_pointer(), "/foo/bar");
    }

    #[test]
    fn test_location_push_index() {
        let mut loc = Location::new();
        loc.push_property("items");
        loc.push_index(3);
        assert_eq!(loc.as_json_pointer(), "/items/3");
    }

    #[test]
    fn test_location_pop() {
        let mut loc = Location::new();
        loc.push_property("foo");
        loc.push_property("bar");
        assert!(loc.pop());
        assert_eq!(loc.as_json_pointer(), "/foo");
    }

    #[test]
    fn test_location_json_pointer_escaping() {
        let mut loc = Location::new();
        loc.push_property("foo/bar");
        loc.push_property("~baz");
        loc.push_index(0);
        // ~0 → ~, ~1 → /
        assert_eq!(loc.as_json_pointer(), "/foo~1bar/~0baz/0");
    }

    #[test]
    fn test_location_display() {
        let mut loc = Location::new();
        loc.push_property("foo");
        assert_eq!(format!("{loc}"), "/foo");
    }

    #[test]
    fn test_location_display_root() {
        let loc = Location::new();
        assert_eq!(format!("{loc}"), "/");
    }

    #[test]
    fn test_lazy_location_materialize() {
        let root = LazyLocation::new();
        let a = root.push_property("a");
        let idx = a.push_index(0);
        let b = idx.push_property("b");
        let loc = b.materialize();
        assert_eq!(loc.as_json_pointer(), "/a/0/b");
    }

    #[test]
    fn test_lazy_location_root_materialize() {
        let root = LazyLocation::new();
        let loc = root.materialize();
        assert!(loc.as_json_pointer().is_empty());
    }

    #[test]
    fn test_lazy_location_display() {
        let root = LazyLocation::new();
        let foo = root.push_property("foo");
        assert_eq!(format!("{foo}"), "/foo");
    }
}
