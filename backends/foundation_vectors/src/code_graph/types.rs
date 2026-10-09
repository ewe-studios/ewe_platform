use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Relation {
    Contains,
    Imports,
    ImportsFrom,
    Inherits,
    Implements,
    Calls,
    Uses,
    Defines,
}

impl core::fmt::Display for Relation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Contains => write!(f, "contains"),
            Self::Imports => write!(f, "imports"),
            Self::ImportsFrom => write!(f, "imports_from"),
            Self::Inherits => write!(f, "inherits"),
            Self::Implements => write!(f, "implements"),
            Self::Calls => write!(f, "calls"),
            Self::Uses => write!(f, "uses"),
            Self::Defines => write!(f, "defines"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Confidence {
    Extracted,
    Inferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    File,
    Module,
    Struct,
    Enum,
    Trait,
    Function,
    Method,
    Impl,
    Constant,
    Static,
    TypeAlias,
    Macro,
}

impl core::fmt::Display for NodeKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::File => write!(f, "file"),
            Self::Module => write!(f, "module"),
            Self::Struct => write!(f, "struct"),
            Self::Enum => write!(f, "enum"),
            Self::Trait => write!(f, "trait"),
            Self::Function => write!(f, "function"),
            Self::Method => write!(f, "method"),
            Self::Impl => write!(f, "impl"),
            Self::Constant => write!(f, "constant"),
            Self::Static => write!(f, "static"),
            Self::TypeAlias => write!(f, "type_alias"),
            Self::Macro => write!(f, "macro"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub kind: NodeKind,
    pub source_file: String,
    pub source_line: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub relation: Relation,
    pub confidence: Confidence,
    pub source_file: String,
    pub source_line: usize,
    pub weight: f32,
}

#[derive(Debug, Clone, Default)]
pub struct FileExtraction {
    pub file_path: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<(String, String, GraphEdge)>,
    pub raw_calls: Vec<RawCall>,
    pub seen_ids: HashMap<String, bool>,
}

#[derive(Debug, Clone)]
pub struct RawCall {
    pub caller_id: String,
    pub callee_name: String,
    pub is_member_call: bool,
    pub source_file: String,
    pub source_line: usize,
}

pub fn make_id(stem: &str, name: &str) -> String {
    let raw = if stem.is_empty() {
        name.to_lowercase()
    } else {
        format!("{}_{}", stem.to_lowercase(), name.to_lowercase())
    };
    raw.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

pub fn file_stem_qualified(path: &str) -> String {
    let p = std::path::Path::new(path);
    let parent = p
        .parent()
        .and_then(|pp| pp.file_name())
        .and_then(|f| f.to_str())
        .unwrap_or("");
    let stem = p
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if parent.is_empty() || parent == "." {
        stem.to_string()
    } else {
        format!("{parent}_{stem}")
    }
}

pub fn normalize_label(label: &str) -> String {
    label
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect()
}
