//! Validation context — mutable state for a single validation run.
//!
//! WHY: Recursive schemas can cause infinite validation loops. The context
//! tracks which (`schema_node`, `instance_location`) pairs are in-progress and
//! caches results to break cycles and avoid redundant work.

use alloc::collections::BTreeSet;
use alloc::string::String;

/// Mutable state for a single validation run.
///
/// WHY: Recursive schemas can cause infinite validation loops. The context
/// tracks which schema/instance pairs are in-progress and caches results.
///
/// HOW: Created fresh for each top-level `is_valid()/validate()` call.
/// Passed as &mut through the entire validation tree.
#[allow(dead_code)]
pub struct ValidationContext {
    /// Active validation paths for cycle detection.
    in_progress: BTreeSet<(usize, String)>,
    /// Memoization cache for recursive reference results.
    cache: BTreeSet<(usize, String)>,
    /// Properties evaluated by sibling keywords.
    evaluated_properties: BTreeSet<String>,
    /// Array indices evaluated by sibling keywords.
    evaluated_items: BTreeSet<usize>,
}

impl ValidationContext {
    /// Create a new empty context.
    #[must_use]
    pub fn new() -> Self {
        Self {
            in_progress: BTreeSet::new(),
            cache: BTreeSet::new(),
            evaluated_properties: BTreeSet::new(),
            evaluated_items: BTreeSet::new(),
        }
    }

    /// Enter a validation at a reference point. Returns false if already
    /// in progress (cycle detected — optimistic resolution).
    #[allow(dead_code)]
    pub fn enter(&mut self, node_id: usize, instance_path: &str) -> bool {
        let key = (node_id, instance_path.to_string());
        if self.in_progress.contains(&key) {
            return false;
        }
        if self.cache.contains(&key) {
            return false;
        }
        self.in_progress.insert(key);
        true
    }

    /// Exit a validation at a reference point.
    #[allow(dead_code)]
    pub fn exit(&mut self, node_id: usize, instance_path: &str, valid: bool) {
        let key = (node_id, instance_path.to_string());
        self.in_progress.remove(&key);
        if valid {
            self.cache.insert(key);
        }
    }

    /// Mark a property as evaluated (for unevaluatedProperties tracking).
    #[allow(dead_code)]
    pub fn mark_property_evaluated(&mut self, name: &str) {
        self.evaluated_properties.insert(name.to_string());
    }

    /// Check if a property was evaluated.
    #[allow(dead_code)]
    #[must_use]
    pub fn is_property_evaluated(&self, name: &str) -> bool {
        self.evaluated_properties.contains(name)
    }

    /// Mark an array index as evaluated (for unevaluatedItems tracking).
    #[allow(dead_code)]
    pub fn mark_item_evaluated(&mut self, index: usize) {
        self.evaluated_items.insert(index);
    }

    /// Check if an array index was evaluated.
    #[allow(dead_code)]
    #[must_use]
    pub fn is_item_evaluated(&self, index: usize) -> bool {
        self.evaluated_items.contains(&index)
    }

    /// Save the current evaluation state.
    #[allow(dead_code)]
    #[must_use]
    pub fn save_evaluation_state(&self) -> EvaluationState {
        EvaluationState {
            evaluated_properties: self.evaluated_properties.clone(),
            evaluated_items: self.evaluated_items.clone(),
        }
    }

    /// Merge evaluation state from a sub-validation.
    #[allow(dead_code)]
    pub fn merge_evaluation_state(&mut self, state: &EvaluationState) {
        self.evaluated_properties
            .extend(state.evaluated_properties.iter().cloned());
        self.evaluated_items
            .extend(state.evaluated_items.iter().copied());
    }

    /// Restore evaluation state to a previous snapshot (discards current marks).
    #[allow(dead_code)]
    pub fn restore_evaluation_state(&mut self, state: &EvaluationState) {
        self.evaluated_properties.clone_from(&state.evaluated_properties);
        self.evaluated_items.clone_from(&state.evaluated_items);
    }
}

impl Default for ValidationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of evaluation state for composition keywords.
#[allow(dead_code)]
#[derive(Clone)]
pub struct EvaluationState {
    evaluated_properties: BTreeSet<String>,
    evaluated_items: BTreeSet<usize>,
}
