//! The agent-rpc Cloudflare deployment plan (spec 83 feature 06, T6).
//!
//! WHY: the agent-rpc Worker deploys through the spec 77
//! `CloudflareDeployer`, the same deployer as the sandbox Worker. This feature
//! supplies the plan; it writes no `wrangler.toml` and it calls no Cloudflare
//! API.
//!
//! WHAT: [`AgentRpcConfig`] (what the operator configures), [`ContainerConfig`]
//! (the native agent-rpc container) and [`agent_rpc_plan`] (the
//! [`DeploymentPlan`]).
//!
//! HOW: the plan names one Worker, the agent instance Durable Object (DO)
//! class, its binding and migration, one D1 database, the optional R2 archive,
//! the routes, the container deployment and one [`SecretRef`] for each secret.
//! A secret is a name and a source only: the plan never holds a value, so the
//! deployer reads the value from its secret store (spec 77 feature 08,
//! invariant I12).

use std::collections::BTreeMap;
use std::path::PathBuf;

use foundation_deployment_cloudflare::{
    Binding, ContainerDeployment, ContainerImage, DeploymentPlan, DoMigration, Route, SecretRef,
    SecretSource, WorkerDeployment,
};

use crate::core::config::SecretRef as ModelSecretRef;
use crate::core::model::ApiModelConfig;

/// The default name of the agent-rpc Worker.
pub const DEFAULT_WORKER_NAME: &str = "cf-agent-rpc";
/// The default `worker-build` output that `wrangler` loads.
pub const DEFAULT_WORKER_MAIN: &str = "build/worker/shim.mjs";
/// The default Cloudflare compatibility date.
pub const DEFAULT_COMPATIBILITY_DATE: &str = "2025-09-01";
/// The default Durable Object class that holds one agent instance.
pub const DEFAULT_INSTANCE_CLASS: &str = "AgentInstance";
/// The default binding of the agent instance Durable Object.
pub const DEFAULT_INSTANCE_BINDING: &str = "AGENT_INSTANCES";
/// The default binding of the D1 trust store and instance index.
pub const DATABASE_BINDING: &str = "AGENT_RPC_DB";
/// The default binding of the R2 history archive.
pub const ARCHIVE_BINDING: &str = "AGENT_RPC_ARCHIVE";
/// The default container application name.
pub const DEFAULT_CONTAINER_NAME: &str = "agent-rpc";
/// The default registry image of the native model server.
pub const DEFAULT_CONTAINER_IMAGE: &str = "ghcr.io/ewe/agent-rpc:latest";
/// The default Cloudflare instance type of the container.
pub const DEFAULT_INSTANCE_TYPE: &str = "lite";
/// The default most container instances of one Worker.
pub const DEFAULT_MAX_INSTANCES: u32 = 10;

/// The native agent-rpc container in the plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainerConfig {
    /// The container application name.
    pub name: String,
    /// The registry image tag. It is ignored when `dockerfile` is set.
    pub image: String,
    /// The image digest, when the continuous integration (CI) job knows it.
    pub digest: Option<String>,
    /// A `Dockerfile` to build and push, when the deploy builds the image.
    pub dockerfile: Option<PathBuf>,
    /// The most container instances that the Worker can have at one time.
    pub max_instances: u32,
    /// The Cloudflare instance type, for example `lite`.
    pub instance_type: String,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            name: DEFAULT_CONTAINER_NAME.to_string(),
            image: DEFAULT_CONTAINER_IMAGE.to_string(),
            digest: None,
            dockerfile: None,
            max_instances: DEFAULT_MAX_INSTANCES,
            instance_type: DEFAULT_INSTANCE_TYPE.to_string(),
        }
    }
}

impl ContainerConfig {
    /// Where the image comes from.
    #[must_use]
    pub fn image(&self) -> ContainerImage {
        match &self.dockerfile {
            Some(path) => ContainerImage::Dockerfile(path.clone()),
            None => ContainerImage::Registry {
                tag: self.image.clone(),
                digest: self.digest.clone(),
            },
        }
    }
}

/// The operator configuration of the agent-rpc Worker.
#[derive(Clone, Debug)]
pub struct AgentRpcConfig {
    /// The Worker name.
    pub worker_name: String,
    /// The Worker module: the `worker-build` output, or a wasm module.
    pub worker_main: PathBuf,
    /// The Cloudflare compatibility date.
    pub compatibility_date: String,
    /// The Durable Object class that holds one agent instance.
    pub instance_class: String,
    /// The binding of the agent instance Durable Object.
    pub instance_binding: String,
    /// The binding of the D1 database.
    pub database_binding: String,
    /// The D1 database identifier, when the database exists.
    pub database_id: Option<String>,
    /// The R2 bucket of the history archive, when the Worker archives.
    pub archive_bucket: Option<String>,
    /// The remote models. Each model names a secret, never a key.
    pub models: Vec<ApiModelConfig>,
    /// The extra secrets that the Worker reads, for example the operator key.
    pub secrets: Vec<SecretRef>,
    /// The route patterns of the Worker.
    pub routes: Vec<String>,
    /// The native agent-rpc container. `None` skips the container deployment.
    pub container: Option<ContainerConfig>,
}

impl Default for AgentRpcConfig {
    fn default() -> Self {
        Self {
            worker_name: DEFAULT_WORKER_NAME.to_string(),
            worker_main: PathBuf::from(DEFAULT_WORKER_MAIN),
            compatibility_date: DEFAULT_COMPATIBILITY_DATE.to_string(),
            instance_class: DEFAULT_INSTANCE_CLASS.to_string(),
            instance_binding: DEFAULT_INSTANCE_BINDING.to_string(),
            database_binding: DATABASE_BINDING.to_string(),
            database_id: None,
            archive_bucket: None,
            models: Vec::new(),
            secrets: Vec::new(),
            routes: Vec::new(),
            container: Some(ContainerConfig::default()),
        }
    }
}

impl AgentRpcConfig {
    /// A configuration with the required values.
    #[must_use]
    pub fn new(worker_name: impl Into<String>, worker_main: impl Into<PathBuf>) -> Self {
        Self {
            worker_name: worker_name.into(),
            worker_main: worker_main.into(),
            ..Self::default()
        }
    }

    /// The secrets of the plan, one for each unique name.
    ///
    /// A model names its secret in `api_key_secret`. A name that appears two
    /// times is one secret; the first source wins. The map is sorted by name,
    /// so the same configuration gives the same plan.
    ///
    /// A malformed JSON reference is not a value: the text becomes a plain
    /// secret name, as a text that is not JSON does. The deployer then reports
    /// the missing secret. The Worker host reports the same error at isolate
    /// start.
    #[must_use]
    pub fn secret_refs(&self) -> Vec<SecretRef> {
        let mut secrets: BTreeMap<String, SecretSource> = BTreeMap::new();
        for model in &self.models {
            let (name, source) = match ModelSecretRef::parse(&model.api_key_secret) {
                Ok(reference) => plan_secret(&reference),
                Err(_) => (model.api_key_secret.clone(), SecretSource::WorkerSecret),
            };
            secrets.entry(name).or_insert(source);
        }
        for secret in &self.secrets {
            secrets
                .entry(secret.name.clone())
                .or_insert(secret.source.clone());
        }
        secrets
            .into_iter()
            .map(|(name, source)| SecretRef { name, source })
            .collect()
    }
}

/// The plan of the agent-rpc Worker.
#[must_use]
pub fn agent_rpc_plan(config: &AgentRpcConfig) -> DeploymentPlan {
    let mut bindings = vec![Binding::DurableObject {
        name: config.instance_binding.clone(),
        class_name: config.instance_class.clone(),
    }];
    if let Some(database_id) = config.database_id.as_ref() {
        bindings.push(Binding::D1 {
            name: config.database_binding.clone(),
            database_id: database_id.clone(),
        });
    }
    if let Some(bucket) = config.archive_bucket.as_ref() {
        bindings.push(Binding::R2 {
            name: ARCHIVE_BINDING.to_string(),
            bucket_name: bucket.clone(),
        });
    }

    let containers = config
        .container
        .as_ref()
        .map(|container| ContainerDeployment {
            class_name: config.instance_class.clone(),
            name: container.name.clone(),
            image: container.image(),
            max_instances: container.max_instances,
            instance_type: container.instance_type.clone(),
        })
        .into_iter()
        .collect();

    let routes = config
        .routes
        .iter()
        .map(|pattern| Route {
            pattern: pattern.clone(),
            zone: None,
        })
        .collect();

    DeploymentPlan {
        worker: WorkerDeployment {
            name: config.worker_name.clone(),
            main: config.worker_main.clone(),
            compatibility_date: config.compatibility_date.clone(),
        },
        bindings,
        migrations: vec![DoMigration {
            tag: "v1".to_string(),
            new_sqlite_classes: vec![config.instance_class.clone()],
        }],
        containers,
        routes,
        secrets: config.secret_refs(),
    }
}

/// Map the model secret reference of the Worker host to the plan reference.
fn plan_secret(reference: &ModelSecretRef) -> (String, SecretSource) {
    match reference {
        ModelSecretRef::WorkerSecret { name } => (name.clone(), SecretSource::WorkerSecret),
        ModelSecretRef::SecretsStore { binding } => (
            binding.clone(),
            SecretSource::SecretsStore {
                store: binding.clone(),
            },
        ),
    }
}
