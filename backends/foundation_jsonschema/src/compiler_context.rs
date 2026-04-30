//! Compiler context — tracks compilation state for recursive schema handling.

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::rc::Rc;
use alloc::string::String;

use core::cell::RefCell;

use crate::draft::Draft;
use crate::error::ValidationError;
use crate::formats::FormatChecker;
use crate::keywords::custom::KeywordFactory;
use crate::paths::Location;
use crate::referencing::{Resolved, Resolver, VocabularySet};

/// Shared set of URIs currently being compiled, for cycle detection.
///
/// WHY: Recursive `$ref` chains like `{"$ref": "#"}` would cause infinite
/// compilation loops. All sub-contexts share this set so a circular reference
/// is detected regardless of how deep the recursion goes.
pub(crate) type InProgress = Rc<RefCell<BTreeSet<String>>>;

/// State tracked during schema compilation.
///
/// WHY: Compiling a JSON Schema requires context: the current base URI,
/// the schema location path for error reporting, the resolver for reference
/// lookup, and cycle detection for recursive schemas.
pub struct CompilerContext<'a> {
    /// Resolver for reference lookup.
    pub resolver: Resolver<'a>,
    /// Current schema location path.
    pub schema_path: Location,
    /// Vocabulary set for the current draft.
    pub vocabulary: VocabularySet,
    /// URIs currently being compiled (cycle detection, shared across sub-contexts).
    pub in_progress: InProgress,
    /// The draft for this schema.
    pub draft: Draft,
    /// Whether format validation should be enforced (vs annotation-only).
    pub assert_format: bool,
    /// Custom format checkers that override built-ins.
    pub custom_formats: BTreeMap<String, Box<dyn FormatChecker>>,
    /// Custom keyword factories for user-defined keywords.
    pub custom_keywords: BTreeMap<String, Box<dyn KeywordFactory>>,
}

impl<'a> CompilerContext<'a> {
    /// Create a new compiler context.
    pub fn new(
        resolver: Resolver<'a>,
        schema_path: Location,
        vocabulary: VocabularySet,
        draft: Draft,
        assert_format: bool,
        custom_formats: BTreeMap<String, Box<dyn FormatChecker>>,
        custom_keywords: BTreeMap<String, Box<dyn KeywordFactory>>,
    ) -> Self {
        Self {
            resolver,
            schema_path,
            vocabulary,
            in_progress: Rc::new(RefCell::new(BTreeSet::new())),
            draft,
            assert_format,
            custom_formats,
            custom_keywords,
        }
    }

    /// Create a sub-context for a keyword.
    pub fn push_keyword(&self, keyword: &str) -> Self {
        let mut new_path = self.schema_path.clone();
        new_path.push_property(keyword);
        Self {
            resolver: self.resolver.clone(),
            schema_path: new_path,
            vocabulary: self.vocabulary.clone(),
            in_progress: Rc::clone(&self.in_progress),
            draft: self.draft,
            assert_format: self.assert_format,
            custom_formats: self.custom_formats.clone(),
            custom_keywords: self.custom_keywords.clone(),
        }
    }

    /// Create a sub-context with a different resolver (for $ref resolution).
    pub fn with_resolver(&self, resolver: Resolver<'a>, keyword: &str) -> Self {
        let mut new_path = self.schema_path.clone();
        new_path.push_property(keyword);
        Self {
            resolver,
            schema_path: new_path,
            vocabulary: self.vocabulary.clone(),
            in_progress: Rc::clone(&self.in_progress),
            draft: self.draft,
            assert_format: self.assert_format,
            custom_formats: self.custom_formats.clone(),
            custom_keywords: self.custom_keywords.clone(),
        }
    }

    /// Resolve a $ref and return the resolved value + new resolver.
    #[allow(dead_code)]
    pub fn resolve_ref(&self, reference: &str) -> Result<Resolved<'a>, ValidationError> {
        self.resolver.lookup(reference)
    }

    /// Check if a URI is already being compiled (cycle detection).
    pub fn is_in_progress(&self, uri: &str) -> bool {
        self.in_progress.borrow().contains(uri)
    }

    /// Mark a URI as being compiled.
    pub fn mark_in_progress(&self, uri: &str) {
        self.in_progress.borrow_mut().insert(uri.to_string());
    }

    /// Unmark a URI after compilation completes.
    pub fn mark_done(&self, uri: &str) {
        self.in_progress.borrow_mut().remove(uri);
    }
}
