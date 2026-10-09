use std::str::FromStr;

use cedar_policy::{Authorizer, Entities, PolicySet, Schema, ValidationMode, Validator};

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

        let policies =
            PolicySet::from_str(policy_text).map_err(|e| CedarError::PolicyParse(e.to_string()))?;

        let validator = Validator::new(schema.clone());
        let result = validator.validate(&policies, ValidationMode::default());

        if !result.validation_passed() {
            let errors: Vec<String> = result.validation_errors().map(|e| e.to_string()).collect();
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
        let policies =
            PolicySet::from_str(policy_text).map_err(|e| CedarError::PolicyParse(e.to_string()))?;

        let schema =
            Schema::from_json_str("{}").map_err(|e| CedarError::SchemaParse(e.to_string()))?;

        Ok(Self {
            schema,
            policies,
            entities: Entities::empty(),
            authorizer: Authorizer::new(),
        })
    }

    #[must_use]
    pub fn is_authorized(&self, request: &CedarRequest) -> CedarResponse {
        let response =
            self.authorizer
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
