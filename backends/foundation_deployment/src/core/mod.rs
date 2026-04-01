//! Provider-agnostic deployment primitives.
//!
//! WHY: All providers share common abstractions — traits, types, shell execution,
//! and project scanning. Centralizing these avoids duplication.
//!
//! WHAT: The `DeploymentProvider` trait, shared output types, `ShellExecutor`
//! for running build commands, and `ProjectScanner` for detecting project metadata.
//!
//! HOW: Each sub-module defines one concern; providers depend on these primitives
//! without importing each other.

pub mod project;
pub mod shell;
pub mod traits;
pub mod types;
