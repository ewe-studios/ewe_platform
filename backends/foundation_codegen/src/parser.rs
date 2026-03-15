use std::path::Path;

use crate::error::{CodegenError, Result};

/// WHY: Each source file must be parsed into an AST before the visitor
/// can search for macro-annotated items.
///
/// WHAT: Reads a `.rs` file and returns its `syn::File` AST.
///
/// HOW: Uses `std::fs::read_to_string` + `syn::parse_file`, which works
/// outside of proc-macro context.
///
/// # Errors
///
/// Returns `CodegenError::Io` if the file cannot be read, or
/// `CodegenError::ParseError` if the source contains invalid Rust syntax.
///
/// # Panics
///
/// Never panics.
pub fn parse_rust_file(path: &Path) -> Result<syn::File> {
    let source = std::fs::read_to_string(path).map_err(|e| CodegenError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    syn::parse_file(&source).map_err(|e| CodegenError::ParseError {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}
