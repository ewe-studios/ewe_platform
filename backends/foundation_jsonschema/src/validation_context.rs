//! Validation context — mutable state for a single validation run.
//!
//! WHY: Recursive schemas can cause infinite validation loops. The context
//! tracks which (`schema_node`, `instance_location`) pairs are in-progress and
//! caches results to break cycles and avoid redundant work.
//!
//! Additionally, unevaluatedProperties/unevaluatedItems need to track which
//! properties/items have been evaluated, but only at the same schema depth.
//! Evaluations from cousin schemas (nested inside allOf/anyOf/oneOf branches)
//! should not be visible.

use alloc::collections::BTreeSet;
use alloc::string::String;

/// Mutable state for a single validation run.
///
/// WHY: Recursive schemas can cause infinite validation loops. The context
/// tracks which schema/instance pairs are in-progress and caches results.
///
/// HOW: Created fresh for each top-level `is_valid()/validate()` call.
/// Passed as &mut through the entire validation tree.
pub struct ValidationContext {
    /// Active validation paths for cycle detection.
    in_progress: BTreeSet<(usize, String)>,
    /// Memoization cache for recursive reference results.
    cache: BTreeSet<(usize, String)>,
    /// Properties evaluated by sibling keywords, tagged with schema depth.
    evaluated_properties: BTreeSet<(u32, String)>,
    /// Array indices evaluated by sibling keywords, tagged with schema depth.
    evaluated_items: BTreeSet<(u32, usize)>,
    /// Current schema depth — incremented when entering a subschema.
    schema_depth: u32,
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
            schema_depth: 0,
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

    /// Increment schema depth (entering a subschema).
    pub fn enter_schema(&mut self) {
        self.schema_depth += 1;
    }

    /// Decrement schema depth (leaving a subschema).
    pub fn exit_schema(&mut self) {
        if self.schema_depth > 0 {
            self.schema_depth -= 1;
        }
    }

    /// Current schema depth.
    #[must_use]
    pub fn current_schema_depth(&self) -> u32 {
        self.schema_depth
    }

    /// Mark a property as evaluated (for unevaluatedProperties tracking).
    pub fn mark_property_evaluated(&mut self, name: &str) {
        self.evaluated_properties
            .insert((self.schema_depth, name.to_string()));
    }

    /// Check if a property was evaluated at the current schema depth.
    #[must_use]
    pub fn is_property_evaluated(&self, name: &str) -> bool {
        self.evaluated_properties
            .contains(&(self.schema_depth, name.to_string()))
    }

    /// Mark an array index as evaluated (for unevaluatedItems tracking).
    pub fn mark_item_evaluated(&mut self, index: usize) {
        self.evaluated_items.insert((self.schema_depth, index));
    }

    /// Check if an array index was evaluated at the current schema depth.
    #[must_use]
    pub fn is_item_evaluated(&self, index: usize) -> bool {
        self.evaluated_items.contains(&(self.schema_depth, index))
    }

    /// Save the current evaluation state.
    #[must_use]
    pub fn save_evaluation_state(&self) -> EvaluationState {
        EvaluationState {
            evaluated_properties: self.evaluated_properties.clone(),
            evaluated_items: self.evaluated_items.clone(),
            schema_depth: self.schema_depth,
        }
    }

    /// Merge evaluation state from a sub-validation (keeping only marks at our depth).
    pub fn merge_evaluation_state(&mut self, state: &EvaluationState) {
        // Only merge marks that are at our current depth
        for (depth, name) in &state.evaluated_properties {
            if *depth == self.schema_depth {
                self.evaluated_properties.insert((*depth, name.clone()));
            }
        }
        for (depth, idx) in &state.evaluated_items {
            if *depth == self.schema_depth {
                self.evaluated_items.insert((*depth, *idx));
            }
        }
    }

    /// Restore evaluation state to a previous snapshot (discards current marks).
    pub fn restore_evaluation_state(&mut self, state: &EvaluationState) {
        self.evaluated_properties
            .clone_from(&state.evaluated_properties);
        self.evaluated_items.clone_from(&state.evaluated_items);
        self.schema_depth = state.schema_depth;
    }
}

impl Default for ValidationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of evaluation state for composition keywords.
#[derive(Clone)]
pub struct EvaluationState {
    evaluated_properties: BTreeSet<(u32, String)>,
    evaluated_items: BTreeSet<(u32, usize)>,
    schema_depth: u32,
}

impl EvaluationState {
    /// Iterate over evaluated properties.
    pub fn evaluated_properties(&self) -> impl Iterator<Item = &(u32, String)> {
        self.evaluated_properties.iter()
    }

    /// Iterate over evaluated items.
    pub fn evaluated_items(&self) -> impl Iterator<Item = &(u32, usize)> {
        self.evaluated_items.iter()
    }
}
