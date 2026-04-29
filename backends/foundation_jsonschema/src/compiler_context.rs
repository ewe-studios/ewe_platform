//! Compiler context — tracks compilation state for recursive schema handling.

use alloc::collections::BTreeSet;
use alloc::string::String;

use crate::draft::Draft;
use crate::error::ValidationError;
use crate::paths::Location;
use crate::referencing::{Resolver, Resolved, VocabularySet};

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
    /// URIs currently being compiled (cycle detection).
    pub in_progress: BTreeSet<String>,
    /// The draft for this schema.
    pub draft: Draft,
}

impl<'a> CompilerContext<'a> {
    /// Create a new compiler context.
    pub fn new(
        resolver: Resolver<'a>,
        schema_path: Location,
        vocabulary: VocabularySet,
        draft: Draft,
    ) -> Self {
        Self {
            resolver,
            schema_path,
            vocabulary,
            in_progress: BTreeSet::new(),
            draft,
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
            in_progress: self.in_progress.clone(),
            draft: self.draft,
        }
    }

    /// Resolve a $ref and return the resolved value + new resolver.
    #[allow(dead_code)]
    pub fn resolve_ref(&self, reference: &str) -> Result<Resolved<'a>, ValidationError> {
        self.resolver.lookup(reference)
    }

    /// Check if a URI is already being compiled.
    #[allow(dead_code)]
    pub fn is_in_progress(&self, uri: &str) -> bool {
        self.in_progress.contains(uri)
    }
}
