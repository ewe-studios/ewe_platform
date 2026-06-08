use std::str::FromStr;

use cedar_policy::{
    Authorizer, Entities, PolicySet, Schema, ValidationMode, Validator,
};

use super::errors::CedarError;
use super::request::CedarRequest;
use super::response::CedarResponse;

pub struct CedarEngine {
    schema: Schema,
    policies: PolicySet,
    entities: Entities,
    authorizer: Authorizer,
}

impl CedarEngine {
    pub fn from_str(schema_text: &str, policy_text: &str) -> Result<Self, CedarError> {
        Self::with_entities(schema_text, policy_text, &Entities::empty())
    }

    pub fn with_entities(
        schema_text: &str,
        policy_text: &str,
        entities: &Entities,
    ) -> Result<Self, CedarError> {
        let (schema, _warnings) = Schema::from_cedarschema_str(schema_text)
            .map_err(|e| CedarError::SchemaParse(e.to_string()))?;

        let policies = PolicySet::from_str(policy_text)
            .map_err(|e| CedarError::PolicyParse(e.to_string()))?;

        let validator = Validator::new(schema.clone());
        let result = validator.validate(&policies, ValidationMode::default());

        if !result.validation_passed() {
            let errors: Vec<String> = result
                .validation_errors()
                .map(|e| e.to_string())
                .collect();
            return Err(CedarError::Validation(errors));
        }

        Ok(Self {
            schema,
            policies,
            entities: entities.clone(),
            authorizer: Authorizer::new(),
        })
    }

    pub fn from_policy_text_only(policy_text: &str) -> Result<Self, CedarError> {
        let policies = PolicySet::from_str(policy_text)
            .map_err(|e| CedarError::PolicyParse(e.to_string()))?;

        let schema = Schema::from_json_str("{}")
            .map_err(|e| CedarError::SchemaParse(e.to_string()))?;

        Ok(Self {
            schema,
            policies,
            entities: Entities::empty(),
            authorizer: Authorizer::new(),
        })
    }

    #[must_use]
    pub fn is_authorized(&self, request: &CedarRequest) -> CedarResponse {
        let response = self
            .authorizer
            .is_authorized(&request.inner, &self.policies, &self.entities);
        CedarResponse::from_response(response)
    }

    pub fn is_authorized_with_entities(
        &self,
        request: &CedarRequest,
        entities: &Entities,
    ) -> CedarResponse {
        let response = self
            .authorizer
            .is_authorized(&request.inner, &self.policies, entities);
        CedarResponse::from_response(response)
    }

    #[must_use]
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    #[must_use]
    pub fn policies(&self) -> &PolicySet {
        &self.policies
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::request::CedarRequest;

    const TEST_SCHEMA: &str = r#"
entity User;
entity Album;
action view appliesTo {
  principal: [User],
  resource: [Album]
};
"#;

    const TEST_POLICY: &str = r#"
permit(
    principal == User::"alice",
    action == Action::"view",
    resource == Album::"trip"
);
"#;

    #[test]
    fn test_engine_from_str() {
        let engine = CedarEngine::from_str(TEST_SCHEMA, TEST_POLICY);
        assert!(engine.is_ok());
    }

    #[test]
    fn test_engine_allows_alice() {
        let engine = CedarEngine::from_str(TEST_SCHEMA, TEST_POLICY).unwrap();
        let req = CedarRequest::builder()
            .principal_str("User", "alice")
            .unwrap()
            .action_str("Action", "view")
            .unwrap()
            .resource_str("Album", "trip")
            .unwrap()
            .build()
            .unwrap();

        let response = engine.is_authorized(&req);
        assert!(response.is_allowed());
        assert!(!response.reasons().is_empty());
    }

    #[test]
    fn test_engine_denies_bob() {
        let engine = CedarEngine::from_str(TEST_SCHEMA, TEST_POLICY).unwrap();
        let req = CedarRequest::builder()
            .principal_str("User", "bob")
            .unwrap()
            .action_str("Action", "view")
            .unwrap()
            .resource_str("Album", "trip")
            .unwrap()
            .build()
            .unwrap();

        let response = engine.is_authorized(&req);
        assert!(!response.is_allowed());
    }

    #[test]
    fn test_engine_invalid_schema() {
        let result = CedarEngine::from_str("invalid{{{", TEST_POLICY);
        assert!(result.is_err());
    }

    #[test]
    fn test_engine_invalid_policy() {
        let result = CedarEngine::from_str(TEST_SCHEMA, "invalid{{{");
        assert!(result.is_err());
    }

    #[test]
    fn test_engine_validation_error() {
        let bad_policy = r#"
permit(
    principal == User::"alice",
    action == Action::"view",
    resource == Album::"trip"
) when { principal.nonexistent_attr > 5 };
"#;
        let result = CedarEngine::from_str(TEST_SCHEMA, bad_policy);
        assert!(result.is_err());
        if let Err(CedarError::Validation(errors)) = result {
            assert!(!errors.is_empty());
        }
    }
}
