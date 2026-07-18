//! Admin handlers for upstream provider management (spec-57, F006).
//!
//! WHY: Operators need CRUD on upstream providers — add new providers, update
//! configs, rotate secrets, deactivate old ones — without direct DB access.
//!
//! WHAT: GET list all providers, GET one, POST create, PUT update, DELETE
//! deactivate, POST set secret. Extended discovery document includes the
//! `providers_supported` field listing available upstream providers.
//!
//! HOW: Free functions taking `ProviderService<impl QueryStore>` + JSON bodies.
//! Registered as routes by `IdpServer::register_routes`.

use serde::{Deserialize, Serialize};

use foundation_netio::shared::http::SendSafeBody;

use super::super::config::IdpConfig;
use super::super::models::provider::{ProviderType, ProviderUpdate, UpstreamProvider};
use super::super::services::provider_service::ProviderService;
use foundation_db::QueryStore;

use super::core::{HandlerResponse, IdpError};

// ── DTOs ──────────────────────────────────────────────────────────────────

/// Public provider representation (client secret never exposed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderEntry {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub scopes: Vec<String>,
    pub is_active: bool,
    pub client_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discovery_url: Option<String>,
}

impl From<&UpstreamProvider> for ProviderEntry {
    fn from(p: &UpstreamProvider) -> Self {
        Self {
            id: p.id.clone(),
            name: p.name.clone(),
            provider_type: p.provider_type.as_str().to_string(),
            scopes: p.scopes.clone(),
            is_active: p.is_active,
            client_id: p.client_id.clone(),
            discovery_url: p.discovery_url.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateProviderRequest {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub provider_type: String,
    pub client_id: String,
    #[serde(default)]
    pub discovery_url: Option<String>,
    #[serde(default)]
    pub authorization_url: Option<String>,
    #[serde(default)]
    pub token_url: Option<String>,
    #[serde(default)]
    pub userinfo_url: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub client_secret: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetSecretRequest {
    pub secret: String,
}

// ── Extended discovery ────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ExtendedDiscovery {
    #[serde(flatten)]
    pub base: super::core::OidcDiscoveryDocument,
    pub providers_supported: Vec<ProviderEntry>,
    pub social_login_endpoint: String,
}

impl ExtendedDiscovery {
    pub fn build(config: &IdpConfig, prefix: &str, providers: &[ProviderEntry]) -> Self {
        let base = config.issuer_url.trim_end_matches('/');
        let p = prefix.trim_end_matches('/');
        Self {
            base: super::core::OidcDiscoveryDocument::from_config(config, prefix),
            providers_supported: providers.to_vec(),
            social_login_endpoint: format!("{base}{p}/authorize"),
        }
    }
}

// ── Handler functions ─────────────────────────────────────────────────────

/// GET /.well-known/openid-configuration with providers.
pub fn extended_discovery(
    config: &IdpConfig,
    prefix: &str,
    providers: &[UpstreamProvider],
) -> Result<HandlerResponse, IdpError> {
    let active: Vec<ProviderEntry> = providers
        .iter()
        .filter(|p| p.is_active)
        .map(ProviderEntry::from)
        .collect();
    let doc = ExtendedDiscovery::build(config, prefix, &active);
    let body = serde_json::to_value(&doc).map_err(|e| IdpError::Internal(e.to_string()))?;
    Ok(HandlerResponse::ok(body))
}

/// GET /admin/providers
pub fn list_all<QS: QueryStore + 'static>(
    svc: &ProviderService<QS>,
) -> Result<HandlerResponse, IdpError> {
    let providers = svc.list_all().map_err(|e| IdpError::Internal(e.to_string()))?;
    let entries: Vec<ProviderEntry> = providers.iter().map(ProviderEntry::from).collect();
    Ok(HandlerResponse::ok(serde_json::json!({ "providers": entries })))
}

/// GET /admin/providers/{id}
pub fn get_one<QS: QueryStore + 'static>(
    svc: &ProviderService<QS>,
    id: &str,
) -> Result<HandlerResponse, IdpError> {
    let provider = svc
        .find_by_id(id)
        .map_err(|e| IdpError::Internal(e.to_string()))?
        .ok_or_else(|| IdpError::BadRequest("provider not found".into()))?;
    Ok(HandlerResponse::ok(serde_json::to_value(ProviderEntry::from(&provider)).unwrap()))
}

/// POST /admin/providers
pub fn create<QS: QueryStore + 'static>(
    svc: &ProviderService<QS>,
    body: CreateProviderRequest,
) -> Result<HandlerResponse, IdpError> {
    let provider_type = match body.provider_type.as_str() {
        "oidc" => ProviderType::Oidc,
        "oauth2" => ProviderType::Oauth2,
        other => return Err(IdpError::BadRequest(format!("unknown provider_type: {other}"))),
    };

    let scopes = if body.scopes.is_empty() {
        vec!["openid".into(), "email".into(), "profile".into()]
    } else {
        body.scopes
    };

    let provider = UpstreamProvider {
        id: body.id,
        name: body.name,
        provider_type,
        client_id: body.client_id,
        client_secret_ciphertext: None,
        encryption_key_id: "default".into(),
        authorization_url: body.authorization_url,
        token_url: body.token_url,
        userinfo_url: body.userinfo_url,
        discovery_url: body.discovery_url,
        scopes,
        is_active: true,
        mapping_config: Default::default(),
        created_at: 0,
        updated_at: 0,
    };

    let created = svc.create(provider).map_err(|e| IdpError::Internal(e.to_string()))?;
    if let Some(secret) = body.client_secret {
        svc.set_secret(&created.id, &secret)
            .map_err(|e| IdpError::Internal(e.to_string()))?;
    }

    Ok(HandlerResponse::ok(serde_json::to_value(ProviderEntry::from(&created)).unwrap()))
}

/// PUT /admin/providers/{id}
pub fn update<QS: QueryStore + 'static>(
    svc: &ProviderService<QS>,
    id: &str,
    body: CreateProviderRequest,
) -> Result<HandlerResponse, IdpError> {
    let provider_type = match body.provider_type.as_str() {
        "oidc" => Some(ProviderType::Oidc),
        "oauth2" => Some(ProviderType::Oauth2),
        _ => None,
    };

    let update = ProviderUpdate {
        name: Some(body.name),
        provider_type,
        client_id: Some(body.client_id),
        authorization_url: Some(body.authorization_url),
        token_url: Some(body.token_url),
        userinfo_url: Some(body.userinfo_url),
        discovery_url: Some(body.discovery_url),
        scopes: Some(body.scopes),
        ..Default::default()
    };

    let updated = svc.update(id, update).map_err(|e| IdpError::Internal(e.to_string()))?;
    Ok(HandlerResponse::ok(serde_json::to_value(ProviderEntry::from(&updated)).unwrap()))
}

/// DELETE /admin/providers/{id}
pub fn delete<QS: QueryStore + 'static>(
    svc: &ProviderService<QS>,
    id: &str,
) -> Result<HandlerResponse, IdpError> {
    svc.delete(id).map_err(|e| IdpError::Internal(e.to_string()))?;
    Ok(HandlerResponse::ok(serde_json::json!({ "deleted": id })))
}

/// POST /admin/providers/{id}/secret
pub fn set_secret<QS: QueryStore + 'static>(
    svc: &ProviderService<QS>,
    id: &str,
    body: SetSecretRequest,
) -> Result<HandlerResponse, IdpError> {
    svc.set_secret(id, &body.secret)
        .map_err(|e| IdpError::Internal(e.to_string()))?;
    Ok(HandlerResponse::ok(serde_json::json!({ "ok": true })))
}

// ── Admin API body parser ────────────────────────────────────────────────

/// Helper: parse JSON from `SendSafeBody`.
pub fn parse_body<T: serde::de::DeserializeOwned>(body: &SendSafeBody) -> Result<T, IdpError> {
    match body {
        SendSafeBody::Bytes(b) => {
            serde_json::from_slice(b).map_err(|e| IdpError::BadRequest(e.to_string()))
        }
        SendSafeBody::Text(t) => {
            serde_json::from_str(t).map_err(|e| IdpError::BadRequest(e.to_string()))
        }
        _ => Err(IdpError::BadRequest("empty body".into())),
    }
}
