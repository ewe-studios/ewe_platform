//! Where a model provider's application programming interface (API) key lives
//! on a Worker (spec 83 feature 05, section 5.11).
//!
//! WHY: a provider API key can spend an account's money. It is a secret, like
//! a private key, and it never enters a var, a log or a record.
//!
//! WHAT: [`SecretRef`] (a Worker secret or an account Secrets Store secret) and
//! [`read_api_keys`].
//!
//! HOW: the model list names the secret, never the key. [`read_api_keys`] reads
//! each secret one time at isolate start, keeps it in a `Zeroizing` string and
//! fails with the secret's name when it is missing.

use std::collections::HashMap;

use crate::core::model::ApiModelConfig;
use crate::error::{AgentRpcError, AgentRpcResult};
use zeroize::Zeroizing;

/// Where one secret lives on the Worker.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SecretRef {
    /// A per-Worker secret (`wrangler secret put <name>`).
    WorkerSecret {
        /// The binding name.
        name: String,
    },
    /// An account-level Secrets Store binding. Read with `get()`.
    SecretsStore {
        /// The binding name.
        binding: String,
    },
}

impl SecretRef {
    /// Read the reference from the `api_key_secret` text.
    ///
    /// A plain name is a Worker secret, as in the earlier configuration. A
    /// text that starts with `{` is a JSON [`SecretRef`].
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] for JSON that is not a [`SecretRef`].
    pub fn parse(text: &str) -> AgentRpcResult<Self> {
        let text = text.trim();
        if text.starts_with('{') {
            serde_json::from_str(text).map_err(|error| {
                AgentRpcError::Config(format!("api_key_secret is not a secret reference: {error}"))
                    .into()
            })
        } else {
            Ok(Self::WorkerSecret {
                name: text.to_string(),
            })
        }
    }

    /// The binding name, for an error message that never shows the value.
    #[must_use]
    pub fn binding(&self) -> &str {
        match self {
            Self::WorkerSecret { name } => name,
            Self::SecretsStore { binding } => binding,
        }
    }
}

/// Read every API key that the model list names.
///
/// A missing secret stops the start with an error that names the secret, never
/// a value.
///
/// # Errors
/// [`AgentRpcError::Config`] for a missing secret or a bad reference.
pub async fn read_api_keys(
    bindings: &worker::Env,
    models: &[ApiModelConfig],
) -> AgentRpcResult<HashMap<String, Zeroizing<String>>> {
    let mut keys = HashMap::new();
    for model in models {
        let reference = SecretRef::parse(&model.api_key_secret)?;
        if keys.contains_key(reference.binding()) {
            continue;
        }
        let value = match &reference {
            SecretRef::WorkerSecret { name } => {
                let secret = bindings.secret(name).map_err(|_| {
                    AgentRpcError::Config(format!(
                        "the Worker secret {name} is missing; set it with `wrangler secret put {name}`"
                    ))
                })?;
                Zeroizing::new(secret.to_string())
            }
            SecretRef::SecretsStore { binding } => {
                let store = bindings.secret_store(binding).map_err(|_| {
                    AgentRpcError::Config(format!("the Secrets Store binding {binding} is missing"))
                })?;
                let value = store.get().await.map_err(|error| {
                    AgentRpcError::Config(format!(
                        "the Secrets Store binding {binding} could not be read: {error}"
                    ))
                })?;
                let Some(value) = value else {
                    return Err(AgentRpcError::Config(format!(
                        "the Secrets Store binding {binding} has no value"
                    ))
                    .into());
                };
                Zeroizing::new(value)
            }
        };
        keys.insert(reference.binding().to_string(), value);
    }
    Ok(keys)
}
