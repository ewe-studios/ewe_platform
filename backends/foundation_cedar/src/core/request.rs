use std::collections::HashMap;
use std::str::FromStr;

use cedar_policy::{Context, EntityId, EntityTypeName, EntityUid, Request, RestrictedExpression};

use super::errors::CedarError;

pub struct CedarRequest {
    pub(crate) inner: Request,
}

pub struct CedarRequestBuilder {
    principal: Option<EntityUid>,
    action: Option<EntityUid>,
    resource: Option<EntityUid>,
    context_pairs: HashMap<String, RestrictedExpression>,
}

impl CedarRequestBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            principal: None,
            action: None,
            resource: None,
            context_pairs: HashMap::new(),
        }
    }

    #[must_use]
    pub fn principal(mut self, uid: EntityUid) -> Self {
        self.principal = Some(uid);
        self
    }

    #[must_use]
    pub fn principal_str(mut self, type_name: &str, id: &str) -> Result<Self, CedarError> {
        let tn = EntityTypeName::from_str(type_name)
            .map_err(|e| CedarError::RequestBuild(e.to_string()))?;
        let eid = EntityId::from_str(id).map_err(|e| CedarError::RequestBuild(e.to_string()))?;
        self.principal = Some(EntityUid::from_type_name_and_id(tn, eid));
        Ok(self)
    }

    #[must_use]
    pub fn action(mut self, uid: EntityUid) -> Self {
        self.action = Some(uid);
        self
    }

    #[must_use]
    pub fn action_str(mut self, type_name: &str, id: &str) -> Result<Self, CedarError> {
        let tn = EntityTypeName::from_str(type_name)
            .map_err(|e| CedarError::RequestBuild(e.to_string()))?;
        let eid = EntityId::from_str(id).map_err(|e| CedarError::RequestBuild(e.to_string()))?;
        self.action = Some(EntityUid::from_type_name_and_id(tn, eid));
        Ok(self)
    }

    #[must_use]
    pub fn resource(mut self, uid: EntityUid) -> Self {
        self.resource = Some(uid);
        self
    }

    #[must_use]
    pub fn resource_str(mut self, type_name: &str, id: &str) -> Result<Self, CedarError> {
        let tn = EntityTypeName::from_str(type_name)
            .map_err(|e| CedarError::RequestBuild(e.to_string()))?;
        let eid = EntityId::from_str(id).map_err(|e| CedarError::RequestBuild(e.to_string()))?;
        self.resource = Some(EntityUid::from_type_name_and_id(tn, eid));
        Ok(self)
    }

    #[must_use]
    pub fn context_json(mut self, json: serde_json::Value) -> Self {
        if let serde_json::Value::Object(map) = json {
            for (k, v) in map {
                if let Ok(expr) = RestrictedExpression::from_str(&v.to_string()) {
                    self.context_pairs.insert(k, expr);
                }
            }
        }
        self
    }

    pub fn build(self) -> Result<CedarRequest, CedarError> {
        let principal = self
            .principal
            .ok_or_else(|| CedarError::RequestBuild("principal is required".into()))?;
        let action = self
            .action
            .ok_or_else(|| CedarError::RequestBuild("action is required".into()))?;
        let resource = self
            .resource
            .ok_or_else(|| CedarError::RequestBuild("resource is required".into()))?;

        let context = if self.context_pairs.is_empty() {
            Context::empty()
        } else {
            Context::from_pairs(self.context_pairs)
                .map_err(|e| CedarError::RequestBuild(e.to_string()))?
        };

        let inner = Request::new(principal, action, resource, context, None)
            .map_err(|e| CedarError::RequestBuild(e.to_string()))?;

        Ok(CedarRequest { inner })
    }
}

impl Default for CedarRequestBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl CedarRequest {
    #[must_use]
    pub fn builder() -> CedarRequestBuilder {
        CedarRequestBuilder::new()
    }
}
