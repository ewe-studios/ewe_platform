use std::collections::HashMap;
use std::path::PathBuf;

/// WHY: Downstream code generators need to know what kind of item was found
/// so they can emit the right code (e.g., struct fields vs enum variants).
///
/// WHAT: Classifies the Rust item kind discovered by the source scanner.
///
/// HOW: Simple enum with `Display` for human-readable output.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ItemKind {
    Struct,
    Enum,
    Trait,
    Function,
    Impl,
    TypeAlias,
}

impl std::fmt::Display for ItemKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Struct => write!(f, "struct"),
            Self::Enum => write!(f, "enum"),
            Self::Trait => write!(f, "trait"),
            Self::Function => write!(f, "fn"),
            Self::Impl => write!(f, "impl"),
            Self::TypeAlias => write!(f, "type"),
        }
    }
}

/// WHY: Error messages and debugging need to pinpoint exactly where
/// an annotated item lives in the source tree.
///
/// WHAT: Records file path, line, and column for a discovered item.
///
/// HOW: Stores absolute path + 1-indexed line/column from `syn::Span`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// Absolute path to the source file
    pub file_path: PathBuf,

    /// Line number (1-indexed)
    pub line: usize,

    /// Column number (1-indexed)
    pub column: usize,
}

impl std::fmt::Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.file_path.display(),
            self.line,
            self.column
        )
    }
}

/// WHY: Macro attributes carry configuration data (e.g., `#[module(name = "auth")]`)
/// that downstream generators need to interpret.
///
/// WHAT: Represents a single parsed value from an attribute's arguments.
///
/// HOW: Enum of the literal types `syn` can parse from attribute meta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeValue {
    /// String literal: `key = "value"`
    String(String),

    /// Boolean literal: `key = true`
    Bool(bool),

    /// Integer literal: `key = 42`
    Int(i64),

    /// Identifier (no quotes): `key = SomeIdent`
    Ident(String),

    /// List of values: `key(a, b, c)`
    List(Vec<AttributeValue>),

    /// Bare flag with no value: `#[module(export)]`
    Flag,
}

/// WHY: This is the primary output consumers use — it bundles everything
/// known about a discovered item so downstream tools can generate code
/// without re-scanning.
///
/// WHAT: A single macro-annotated item with its full metadata.
///
/// HOW: Assembled by combining `FoundItem` (from the visitor) with
/// `CrateMetadata` and resolved module paths.
#[derive(Debug, Clone)]
pub struct DerivedTarget {
    /// The name of the macro attribute that was found (e.g., "module")
    pub macro_name: String,

    /// Parsed attribute arguments as key-value pairs
    pub attributes: HashMap<String, AttributeValue>,

    /// The name of the item (e.g., `AuthHandler`)
    pub item_name: String,

    /// What kind of item this is (struct, enum, trait, etc.)
    pub item_kind: ItemKind,

    /// Source location (file, line, column)
    pub location: Location,

    /// Full module path (e.g., `my_crate::handlers::auth`)
    pub module_path: String,

    /// Fully qualified path including item name
    pub qualified_path: String,

    /// The crate name this item belongs to
    pub crate_name: String,

    /// Root directory of the crate
    pub crate_root: PathBuf,

    /// Path to the crate's Cargo.toml
    pub cargo_toml_path: PathBuf,
}

/// WHY: The scanner needs crate-level context (name, paths) to resolve
/// module paths and attach metadata to discovered items.
///
/// WHAT: Information extracted from a crate's `Cargo.toml`.
///
/// HOW: Parsed via `toml` crate from the `[package]` and `[lib]` sections.
#[derive(Debug, Clone)]
pub struct CrateMetadata {
    /// Crate name from `[package].name`
    pub name: String,

    /// Crate version from `[package].version`
    pub version: String,

    /// Path to the crate root directory (parent of `Cargo.toml`)
    pub root_dir: PathBuf,

    /// Path to `Cargo.toml`
    pub cargo_toml_path: PathBuf,

    /// Path to the src directory
    pub src_dir: PathBuf,

    /// Crate entry point (`lib.rs` or `main.rs`)
    pub entry_point: PathBuf,

    /// Whether this is a library crate (has `lib.rs`)
    pub is_lib: bool,
}

/// WHY: The AST visitor produces intermediate results before module path
/// resolution happens — this type captures what was found in a single file
/// without requiring crate-level context.
///
/// WHAT: An item found by the `MacroFinder` visitor, before module path resolution.
///
/// HOW: Created by the visitor when an attribute matching the target name
/// is found on a struct/enum/trait/fn/type/impl item.
#[derive(Debug, Clone)]
pub struct FoundItem {
    /// The item name (e.g., `AuthHandler`)
    pub item_name: String,

    /// What kind of item (struct, enum, trait, fn, etc.)
    pub item_kind: ItemKind,

    /// Parsed attribute arguments
    pub attributes: HashMap<String, AttributeValue>,

    /// Source location
    pub location: Location,

    /// The macro attribute name that matched
    pub macro_name: String,

    /// Module nesting within this file (from inline `mod` blocks).
    /// E.g., if struct is inside `mod inner { mod deep { ... } }`,
    /// this would be `["inner", "deep"]`.
    pub inline_module_path: Vec<String>,
}
