//! Evaluation output — structured validation results.
//!
//! WHY: JSON Schema defines three output formats: flag, list, and hierarchical.
//! This module provides the types and methods to produce each format.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

/// The result of evaluating a JSON instance against a compiled schema.
///
/// Provides access to the three JSON Schema Output formats:
/// flag, list, and hierarchical.
#[derive(Debug, Clone)]
pub struct Evaluation {
    root: EvaluationNode,
}

/// A single node in the evaluation tree.
#[derive(Debug, Clone)]
pub struct EvaluationNode {
    /// Whether this node passed validation.
    pub valid: bool,
    /// JSON Pointer path to the evaluation node.
    pub evaluation_path: String,
    /// JSON Pointer path to the schema location.
    pub schema_location: String,
    /// JSON Pointer path to the instance location.
    pub instance_location: String,
    /// Validation errors found at this node.
    pub errors: Vec<String>,
    /// Annotation values collected at this node.
    pub annotations: BTreeMap<String, String>,
    /// Child evaluation nodes.
    pub children: Vec<EvaluationNode>,
}

impl Evaluation {
    /// Create an evaluation from a root node.
    #[must_use]
    pub fn new(root: EvaluationNode) -> Self {
        Self { root }
    }

    /// Flag output: simple valid/invalid boolean.
    #[must_use]
    pub fn valid(&self) -> bool {
        self.root.valid
    }

    /// List output: flat list of all evaluation entries.
    #[must_use]
    pub fn to_list(&self) -> ListOutput {
        let mut entries = Vec::new();
        flatten_node(&self.root, &mut entries);
        ListOutput {
            valid: self.root.valid,
            entries,
        }
    }

    /// Hierarchical output: nested tree.
    #[must_use]
    pub fn to_hierarchical(&self) -> HierarchicalOutput {
        node_to_hierarchical(&self.root)
    }
}

impl EvaluationNode {
    /// Create a new evaluation node.
    #[must_use]
    pub fn new(
        valid: bool,
        evaluation_path: String,
        schema_location: String,
        instance_location: String,
    ) -> Self {
        Self {
            valid,
            evaluation_path,
            schema_location,
            instance_location,
            errors: Vec::new(),
            annotations: BTreeMap::new(),
            children: Vec::new(),
        }
    }
}

/// Flag output — simple boolean.
#[derive(Debug, Clone)]
pub struct FlagOutput {
    /// Whether the overall evaluation passed.
    pub valid: bool,
}

/// List output — flat list of evaluation entries.
#[derive(Debug, Clone)]
pub struct ListOutput {
    /// Whether the overall evaluation passed.
    pub valid: bool,
    /// The flat list of evaluation entries.
    pub entries: Vec<ListEntry>,
}

/// A single entry in the list output.
#[derive(Debug, Clone)]
pub struct ListEntry {
    /// Whether this entry passed validation.
    pub valid: bool,
    /// JSON Pointer path to the evaluation node.
    pub evaluation_path: String,
    /// JSON Pointer path to the schema location.
    pub schema_location: String,
    /// JSON Pointer path to the instance location.
    pub instance_location: String,
    /// Validation errors found at this entry.
    pub errors: Vec<String>,
}

/// Hierarchical output — nested tree.
#[derive(Debug, Clone)]
pub struct HierarchicalOutput {
    /// Whether this node passed validation.
    pub valid: bool,
    /// JSON Pointer path to the evaluation node.
    pub evaluation_path: String,
    /// JSON Pointer path to the schema location.
    pub schema_location: String,
    /// JSON Pointer path to the instance location.
    pub instance_location: String,
    /// Validation errors found at this node.
    pub errors: Vec<String>,
    /// Nested child output nodes.
    pub children: Vec<HierarchicalOutput>,
}

fn flatten_node(node: &EvaluationNode, out: &mut Vec<ListEntry>) {
    out.push(ListEntry {
        valid: node.valid,
        evaluation_path: node.evaluation_path.clone(),
        schema_location: node.schema_location.clone(),
        instance_location: node.instance_location.clone(),
        errors: node.errors.clone(),
    });
    for child in &node.children {
        flatten_node(child, out);
    }
}

fn node_to_hierarchical(node: &EvaluationNode) -> HierarchicalOutput {
    HierarchicalOutput {
        valid: node.valid,
        evaluation_path: node.evaluation_path.clone(),
        schema_location: node.schema_location.clone(),
        instance_location: node.instance_location.clone(),
        errors: node.errors.clone(),
        children: node.children.iter().map(node_to_hierarchical).collect(),
    }
}
