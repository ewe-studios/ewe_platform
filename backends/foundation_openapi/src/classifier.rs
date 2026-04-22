//! Operation type classification for API endpoints.
//!
//! WHY: Provider wrappers need to know which operations modify state (wrap with `StoreStateIdentifierTask`)
//!      and which are read-only (simple execute without state tracking).
//!
//! WHAT: `OperationTypeClassifier` with keyword-based and method/path-based classification.
//!
//! HOW: Analyzes `operation_id` keywords first, falls back to HTTP method and path patterns.

use crate::endpoint::{EndpointInfo, OperationEffect, OperationType};

/// Classifies endpoint operation type based on heuristics.
pub struct OperationTypeClassifier;

impl OperationTypeClassifier {
    /// Classify an endpoint's operation type.
    #[must_use]
    pub fn classify(endpoint: &EndpointInfo) -> OperationType {
        // Check operation_id keywords first
        if let Some(op_type) = Self::classify_by_operation_id(&endpoint.operation_id) {
            return op_type;
        }

        // Fall back to HTTP method and path analysis
        Self::classify_by_method_and_path(endpoint)
    }

    /// Classify by `operation_id` keywords.
    fn classify_by_operation_id(operation_id: &str) -> Option<OperationType> {
        let lower = operation_id.to_lowercase();

        // Create keywords
        if lower.contains("create")
            || lower.contains("insert")
            || lower.contains("add")
            || lower.contains("register")
            || lower.contains("new")
            || lower.contains("allocate")
            || lower.contains("provision")
        {
            return Some(OperationType::Create);
        }

        // Read keywords - informational operations only
        if lower.contains("get")
            || lower.contains("list")
            || lower.contains("fetch")
            || lower.contains("retrieve")
            || lower.contains("search")
            || lower.contains("query")
            || lower.contains("describe")
            || lower.contains("inspect")
        {
            return Some(OperationType::Read);
        }

        // Update keywords
        if lower.contains("update")
            || lower.contains("patch")
            || lower.contains("modify")
            || lower.contains("replace")
            || lower.contains("set")
            || lower.contains("configure")
        {
            return Some(OperationType::Update);
        }

        // Delete keywords
        if lower.contains("delete")
            || lower.contains("remove")
            || lower.contains("destroy")
            || lower.contains("unpublish")
            || lower.contains("deprovision")
        {
            return Some(OperationType::Delete);
        }

        // Action keywords - need further classification
        if lower.contains("cancel")
            || lower.contains("start")
            || lower.contains("stop")
            || lower.contains("restart")
            || lower.contains("activate")
            || lower.contains("deactivate")
            || lower.contains("enable")
            || lower.contains("disable")
            || lower.contains("trigger")
            || lower.contains("invoke")
            || lower.contains("execute")
            || lower.contains("run")
            || lower.contains("test")
            || lower.contains("validate")
            || lower.contains("export")
            || lower.contains("analyze")
            || lower.contains("watch")
        {
            // Classify action as mutating or read-only
            let effect = Self::classify_action_effect(&lower);
            return Some(OperationType::Action(effect));
        }

        None
    }

    /// Classify action effect as mutating or read-only.
    fn classify_action_effect(action_lower: &str) -> OperationEffect {
        // Mutating actions - state-changing operations
        if action_lower.contains("cancel")
            || action_lower.contains("start")
            || action_lower.contains("stop")
            || action_lower.contains("restart")
            || action_lower.contains("activate")
            || action_lower.contains("deactivate")
            || action_lower.contains("enable")
            || action_lower.contains("disable")
            || action_lower.contains("trigger")
            || action_lower.contains("invoke")
            || action_lower.contains("execute")
            || action_lower.contains("run")
        {
            return OperationEffect::Mutating;
        }

        // Read-only actions - informational operations
        if action_lower.contains("test")
            || action_lower.contains("validate")
            || action_lower.contains("export")
            || action_lower.contains("analyze")
            || action_lower.contains("inspect")
            || action_lower.contains("watch")
        {
            return OperationEffect::ReadOnly;
        }

        // Default to mutating for unknown actions (conservative)
        OperationEffect::Mutating
    }

    /// Classify by HTTP method and path patterns as fallback.
    fn classify_by_method_and_path(endpoint: &EndpointInfo) -> OperationType {
        match endpoint.method.as_str() {
            "GET" => OperationType::Read,
            "DELETE" => OperationType::Delete,
            "PUT" | "PATCH" => OperationType::Update,
            "POST" => {
                // Check if path has resource ID (update) or is collection (create)
                if endpoint.path.contains('{') {
                    OperationType::Update
                } else {
                    OperationType::Create
                }
            }
            _ => OperationType::Action(OperationEffect::Mutating),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_classify_create_operations() {
        let endpoint = EndpointInfo {
            operation_id: "createInstance".to_string(),
            method: "POST".to_string(),
            path: "/v1/instances".to_string(),
            path_params: vec![],
            query_params: vec![],
            request_type: None,
            response_type: None,
            error_types: BTreeMap::new(),
            success_codes: vec![],
            base_url: None,
            summary: None,
            operation_type: OperationType::Create,
        };
        assert_eq!(
            OperationTypeClassifier::classify(&endpoint),
            OperationType::Create
        );

        let endpoint = EndpointInfo {
            operation_id: "insertRow".to_string(),
            method: "POST".to_string(),
            path: "/v1/rows".to_string(),
            path_params: vec![],
            query_params: vec![],
            request_type: None,
            response_type: None,
            error_types: BTreeMap::new(),
            success_codes: vec![],
            base_url: None,
            summary: None,
            operation_type: OperationType::Create,
        };
        assert_eq!(
            OperationTypeClassifier::classify(&endpoint),
            OperationType::Create
        );
    }

    #[test]
    fn test_classify_read_operations() {
        let endpoint = EndpointInfo {
            operation_id: "getInstance".to_string(),
            method: "GET".to_string(),
            path: "/v1/instances/{id}".to_string(),
            path_params: vec!["id".to_string()],
            query_params: vec![],
            request_type: None,
            response_type: None,
            error_types: BTreeMap::new(),
            success_codes: vec![],
            base_url: None,
            summary: None,
            operation_type: OperationType::Read,
        };
        assert_eq!(
            OperationTypeClassifier::classify(&endpoint),
            OperationType::Read
        );

        let endpoint = EndpointInfo {
            operation_id: "listInstances".to_string(),
            method: "GET".to_string(),
            path: "/v1/instances".to_string(),
            path_params: vec![],
            query_params: vec![],
            request_type: None,
            response_type: None,
            error_types: BTreeMap::new(),
            success_codes: vec![],
            base_url: None,
            summary: None,
            operation_type: OperationType::Read,
        };
        assert_eq!(
            OperationTypeClassifier::classify(&endpoint),
            OperationType::Read
        );
    }

    #[test]
    fn test_classify_update_operations() {
        let endpoint = EndpointInfo {
            operation_id: "updateInstance".to_string(),
            method: "PATCH".to_string(),
            path: "/v1/instances/{id}".to_string(),
            path_params: vec!["id".to_string()],
            query_params: vec![],
            request_type: None,
            response_type: None,
            error_types: BTreeMap::new(),
            success_codes: vec![],
            base_url: None,
            summary: None,
            operation_type: OperationType::Update,
        };
        assert_eq!(
            OperationTypeClassifier::classify(&endpoint),
            OperationType::Update
        );
    }

    #[test]
    fn test_classify_delete_operations() {
        let endpoint = EndpointInfo {
            operation_id: "deleteInstance".to_string(),
            method: "DELETE".to_string(),
            path: "/v1/instances/{id}".to_string(),
            path_params: vec!["id".to_string()],
            query_params: vec![],
            request_type: None,
            response_type: None,
            error_types: BTreeMap::new(),
            success_codes: vec![],
            base_url: None,
            summary: None,
            operation_type: OperationType::Delete,
        };
        assert_eq!(
            OperationTypeClassifier::classify(&endpoint),
            OperationType::Delete
        );
    }

    #[test]
    fn test_classify_mutating_actions() {
        let endpoint = EndpointInfo {
            operation_id: "startInstance".to_string(),
            method: "POST".to_string(),
            path: "/v1/instances/{id}:start".to_string(),
            path_params: vec!["id".to_string()],
            query_params: vec![],
            request_type: None,
            response_type: None,
            error_types: BTreeMap::new(),
            success_codes: vec![],
            base_url: None,
            summary: None,
            operation_type: OperationType::Action(OperationEffect::Mutating),
        };
        assert_eq!(
            OperationTypeClassifier::classify(&endpoint),
            OperationType::Action(OperationEffect::Mutating)
        );

        let endpoint = EndpointInfo {
            operation_id: "cancelOperation".to_string(),
            method: "POST".to_string(),
            path: "/v1/operations/{id}:cancel".to_string(),
            path_params: vec!["id".to_string()],
            query_params: vec![],
            request_type: None,
            response_type: None,
            error_types: BTreeMap::new(),
            success_codes: vec![],
            base_url: None,
            summary: None,
            operation_type: OperationType::Action(OperationEffect::Mutating),
        };
        assert_eq!(
            OperationTypeClassifier::classify(&endpoint),
            OperationType::Action(OperationEffect::Mutating)
        );
    }

    #[test]
    fn test_classify_readonly_actions() {
        let endpoint = EndpointInfo {
            operation_id: "testIamPermissions".to_string(),
            method: "POST".to_string(),
            path: "/v1/resources/{id}:testIamPermissions".to_string(),
            path_params: vec!["id".to_string()],
            query_params: vec![],
            request_type: None,
            response_type: None,
            error_types: BTreeMap::new(),
            success_codes: vec![],
            base_url: None,
            summary: None,
            operation_type: OperationType::Action(OperationEffect::ReadOnly),
        };
        assert_eq!(
            OperationTypeClassifier::classify(&endpoint),
            OperationType::Action(OperationEffect::ReadOnly)
        );
    }

    #[test]
    fn test_classify_by_method_fallback() {
        // GET without keyword -> Read
        let endpoint = EndpointInfo {
            operation_id: "v1Compute".to_string(),
            method: "GET".to_string(),
            path: "/v1/compute".to_string(),
            path_params: vec![],
            query_params: vec![],
            request_type: None,
            response_type: None,
            error_types: BTreeMap::new(),
            success_codes: vec![],
            base_url: None,
            summary: None,
            operation_type: OperationType::Read,
        };
        assert_eq!(
            OperationTypeClassifier::classify(&endpoint),
            OperationType::Read
        );

        // DELETE without keyword -> Delete
        let endpoint = EndpointInfo {
            operation_id: "v1Compute".to_string(),
            method: "DELETE".to_string(),
            path: "/v1/compute/{id}".to_string(),
            path_params: vec!["id".to_string()],
            query_params: vec![],
            request_type: None,
            response_type: None,
            error_types: BTreeMap::new(),
            success_codes: vec![],
            base_url: None,
            summary: None,
            operation_type: OperationType::Delete,
        };
        assert_eq!(
            OperationTypeClassifier::classify(&endpoint),
            OperationType::Delete
        );
    }
}
