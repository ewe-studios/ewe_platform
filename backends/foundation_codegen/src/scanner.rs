use std::path::Path;

use syn::visit::Visit;

use crate::error::{CodegenError, Result};
use crate::file_walker::find_rust_files;
use crate::parser::parse_rust_file;
use crate::types::FoundItem;
use crate::visitor::MacroFinder;

/// WHY: Consumers need a simple entry point to scan one or many files
/// for macro-annotated items without managing the walker/parser/visitor
/// pipeline themselves.
///
/// WHAT: Public API that ties together file walking, parsing, and the
/// `MacroFinder` visitor into `scan_file` and `scan_directory` methods.
///
/// HOW: Holds the target attribute name; each scan method creates a fresh
/// `MacroFinder` per file and collects results.
pub struct SourceScanner {
    target_attr: String,
}

impl SourceScanner {
    /// WHY: The scanner is generic over any attribute name so it is not
    /// hardcoded to a single macro.
    ///
    /// WHAT: Creates a new scanner that searches for the given attribute.
    ///
    /// HOW: Stores the attribute name for use in each `MacroFinder`.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn new(target_attr: &str) -> Self {
        Self {
            target_attr: target_attr.to_string(),
        }
    }

    /// WHY: Some consumers only need to scan a single file (e.g., incremental
    /// builds or editor integrations).
    ///
    /// WHAT: Parses one `.rs` file and returns all items annotated with the
    /// target attribute.
    ///
    /// HOW: Parses with `syn`, runs `MacroFinder`, returns collected items.
    ///
    /// # Errors
    ///
    /// Returns `CodegenError::Io` if the file cannot be read, or
    /// `CodegenError::ParseError` if the source is invalid.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn scan_file(&self, file_path: &Path) -> Result<Vec<FoundItem>> {
        let ast = parse_rust_file(file_path)?;
        let file_path_buf = file_path.to_path_buf();
        let mut finder = MacroFinder::new(&self.target_attr, &file_path_buf);
        finder.visit_file(&ast);
        Ok(finder.found)
    }

    /// WHY: The primary use case is scanning an entire crate's `src/`
    /// directory to find all annotated items.
    ///
    /// WHAT: Recursively scans all `.rs` files in a directory, collecting
    /// annotated items. Files that fail to parse are skipped with a warning.
    ///
    /// HOW: Uses `find_rust_files` for discovery, then `scan_file` per file.
    /// Parse errors are logged to stderr but do not abort the scan.
    ///
    /// # Errors
    ///
    /// Returns `CodegenError::Io` if the directory cannot be walked.
    /// Parse errors for individual files are tolerated (logged, not returned).
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn scan_directory(&self, src_dir: &Path) -> Result<Vec<FoundItem>> {
        let files = find_rust_files(src_dir)?;
        let mut all_found = Vec::new();

        for file in files {
            match self.scan_file(&file) {
                Ok(found) => all_found.extend(found),
                Err(CodegenError::ParseError { path, message }) => {
                    eprintln!("Warning: failed to parse {}: {message}", path.display());
                }
                Err(e) => return Err(e),
            }
        }

        Ok(all_found)
    }
}
