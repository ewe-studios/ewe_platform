//! The deployment plan of one Cloudflare Worker.
//!
//! The plan is plain data with no side effect. Both deploy adapters read the
//! same plan: the Rust adapter applies it, and the `wrangler` adapter writes it
//! as a `wrangler.toml`. The plan holds secret names and sources, never secret
//! values.

use std::fmt;
use std::path::PathBuf;

use foundation_compact::rng::Rng;
use foundation_context::Ctx;
use serde::{Deserialize, Serialize};

use crate::deploy::DeployError;

/// The one description of a Worker, its bindings, migrations, containers,
/// routes and secrets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeploymentPlan {
    /// The Worker itself.
    pub worker: WorkerDeployment,
    /// The bindings of the Worker.
    pub bindings: Vec<Binding>,
    /// The Durable Object (DO) migrations.
    pub migrations: Vec<DoMigration>,
    /// The containers of the Worker.
    pub containers: Vec<ContainerDeployment>,
    /// The routes of the Worker.
    pub routes: Vec<Route>,
    /// The names and sources of the Worker secrets.
    pub secrets: Vec<SecretRef>,
}

/// One Worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerDeployment {
    /// The Worker name, for example `ewe-sandbox`.
    pub name: String,
    /// The Worker module: the `worker-build` output, or a WebAssembly (wasm)
    /// module.
    pub main: PathBuf,
    /// The compatibility date.
    pub compatibility_date: String,
}

/// One binding of the Worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Binding {
    /// A Durable Object class.
    DurableObject {
        /// The binding name.
        name: String,
        /// The Durable Object class name.
        class_name: String,
    },
    /// A D1 database.
    D1 {
        /// The binding name.
        name: String,
        /// The database identifier.
        database_id: String,
    },
    /// A Key Value (KV) namespace.
    Kv {
        /// The binding name.
        name: String,
        /// The namespace identifier.
        namespace_id: String,
    },
    /// An R2 bucket.
    R2 {
        /// The binding name.
        name: String,
        /// The bucket name.
        bucket_name: String,
    },
    /// The Cloudflare Model API binding.
    Ai {
        /// The binding name.
        name: String,
    },
    /// Another Worker service.
    Service {
        /// The binding name.
        name: String,
        /// The service name.
        service: String,
    },
    /// A plain text value. It is never a secret.
    PlainText {
        /// The binding name.
        name: String,
        /// The value.
        value: String,
    },
}

impl Binding {
    /// The binding name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::DurableObject { name, .. }
            | Self::D1 { name, .. }
            | Self::Kv { name, .. }
            | Self::R2 { name, .. }
            | Self::Ai { name }
            | Self::Service { name, .. }
            | Self::PlainText { name, .. } => name,
        }
    }

    /// The Durable Object class name, when this binding is one.
    #[must_use]
    pub fn class_name(&self) -> Option<&str> {
        match self {
            Self::DurableObject { class_name, .. } => Some(class_name),
            _ => None,
        }
    }
}

/// One Durable Object migration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DoMigration {
    /// The migration tag.
    pub tag: String,
    /// The classes that the migration creates with a Structured Query Language
    /// (SQL) store.
    pub new_sqlite_classes: Vec<String>,
}

/// One container of the Worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainerDeployment {
    /// The Durable Object class that owns the container.
    pub class_name: String,
    /// The container application name.
    pub name: String,
    /// Where the image comes from.
    pub image: ContainerImage,
    /// The most container instances that the Worker can have at one time.
    pub max_instances: u32,
    /// The instance type, for example `lite`.
    pub instance_type: String,
}

/// Where a container image comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContainerImage {
    /// Build and push the image from this `Dockerfile`.
    Dockerfile(PathBuf),
    /// A registry image that a continuous integration (CI) job built.
    Registry {
        /// The image tag.
        tag: String,
        /// The image digest, when the caller knows it.
        digest: Option<String>,
    },
}

/// One route of the Worker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    /// The route pattern.
    pub pattern: String,
    /// The zone, when the route needs one.
    pub zone: Option<String>,
}

/// One secret of the Worker. It holds a name and a source, never a value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretRef {
    /// The secret name.
    pub name: String,
    /// Where the value comes from.
    pub source: SecretSource,
}

/// Where a secret value comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecretSource {
    /// A Worker secret.
    WorkerSecret,
    /// A secret in a Cloudflare Secrets Store.
    SecretsStore {
        /// The store name.
        store: String,
    },
}

/// A deployment identifier.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeploymentId(String);

impl DeploymentId {
    /// An identifier from a value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// A new identifier from the random number generator of `ctx`.
    ///
    /// # Panics
    /// When the `Ctx` random number generator lock is poisoned.
    #[must_use]
    pub fn generate(ctx: &Ctx) -> Self {
        let value = ctx
            .rng()
            .lock()
            .expect("the Ctx RNG lock is not poisoned")
            .next_u64();
        Self(format!("{value:016x}"))
    }

    /// The identifier as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeploymentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The state of one deployment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeploymentStatus {
    /// The deployment waits.
    Queued,
    /// The Worker or the image builds.
    Building,
    /// The Worker version uploads.
    Uploading,
    /// The deployment serves traffic.
    Live,
    /// The deployment failed.
    Failed,
    /// The deployment rolled back.
    RolledBack,
}

/// What one successful deploy left behind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeploymentOutcome {
    /// The deployment identifier.
    pub id: DeploymentId,
    /// The Worker URL.
    pub worker_url: String,
    /// The container tags that the deploy used.
    pub container_tags: Vec<String>,
    /// The deploy time as Unix milliseconds.
    pub deployed_unix_ms: u64,
}

impl DeploymentPlan {
    /// The plan of the sandbox Worker: the Worker, its three Durable Object
    /// classes, the model slave binding, the Model API binding, the container
    /// and the `SANDBOX_TOKEN` secret.
    ///
    /// The model key is not in this plan. Add it with [`Self::with_open_router_key`]
    /// when the model backend is `OpenRouter`.
    #[must_use]
    pub fn sandbox_worker() -> Self {
        Self {
            worker: WorkerDeployment {
                name: "ewe-sandbox".to_string(),
                main: PathBuf::from("build/index.js"),
                compatibility_date: "2026-05-12".to_string(),
            },
            bindings: vec![
                Binding::DurableObject {
                    name: "SANDBOX".to_string(),
                    class_name: "SandboxDo".to_string(),
                },
                Binding::DurableObject {
                    name: "SANDBOX_INDEX".to_string(),
                    class_name: "SandboxIndexDo".to_string(),
                },
                Binding::DurableObject {
                    name: "SLAVE".to_string(),
                    class_name: "ModelSlaveDo".to_string(),
                },
                Binding::Ai {
                    name: "AI".to_string(),
                },
            ],
            migrations: vec![DoMigration {
                tag: "v1".to_string(),
                new_sqlite_classes: vec![
                    "SandboxDo".to_string(),
                    "SandboxIndexDo".to_string(),
                    "ModelSlaveDo".to_string(),
                ],
            }],
            containers: vec![ContainerDeployment {
                class_name: "SandboxDo".to_string(),
                name: "ewe-sandbox".to_string(),
                image: ContainerImage::Dockerfile(PathBuf::from("Dockerfile")),
                max_instances: 10,
                instance_type: "lite".to_string(),
            }],
            routes: Vec::new(),
            secrets: vec![SecretRef {
                name: "SANDBOX_TOKEN".to_string(),
                source: SecretSource::WorkerSecret,
            }],
        }
    }

    /// The plan of the sandbox Worker with a product image.
    ///
    /// It is [`Self::sandbox_worker`] with one change: the container image is
    /// `./Dockerfile.product`, the image that
    /// `mise run sandbox:product-dockerfile` writes. The Worker, its bindings,
    /// its migrations and its secret are the same.
    #[must_use]
    pub fn sandbox_worker_product() -> Self {
        let mut plan = Self::sandbox_worker();
        plan.containers[0].image =
            ContainerImage::Dockerfile(PathBuf::from("./Dockerfile.product"));
        plan
    }

    /// The same plan with the `MODEL_API_KEY` secret, for the `OpenRouter`
    /// model backend.
    #[must_use]
    pub fn with_open_router_key(mut self) -> Self {
        if !self
            .secrets
            .iter()
            .any(|secret| secret.name == "MODEL_API_KEY")
        {
            self.secrets.push(SecretRef {
                name: "MODEL_API_KEY".to_string(),
                source: SecretSource::WorkerSecret,
            });
        }
        self
    }
}

/// Validates one plan.
///
/// The rules: one Worker name and one main module, no duplicate binding name,
/// no duplicate migration tag, every container class is a Durable Object
/// binding, and every Durable Object class is in a migration.
///
/// # Errors
/// [`DeployError::InvalidPlan`] for the first rule that fails.
pub fn validate(plan: &DeploymentPlan) -> Result<(), DeployError> {
    if plan.worker.name.trim().is_empty() {
        return Err(invalid("worker.name", "the Worker name is empty"));
    }
    if plan.worker.main.as_os_str().is_empty() {
        return Err(invalid("worker.main", "the main module is empty"));
    }
    if plan.worker.compatibility_date.trim().is_empty() {
        return Err(invalid(
            "worker.compatibility_date",
            "the compatibility date is empty",
        ));
    }
    for (index, binding) in plan.bindings.iter().enumerate() {
        if plan.bindings[..index]
            .iter()
            .any(|other| other.name() == binding.name())
        {
            return Err(invalid(
                "bindings",
                &format!("the binding name {} is not unique", binding.name()),
            ));
        }
    }
    for (index, migration) in plan.migrations.iter().enumerate() {
        if plan.migrations[..index]
            .iter()
            .any(|other| other.tag == migration.tag)
        {
            return Err(invalid(
                "migrations",
                &format!("the migration tag {} is not unique", migration.tag),
            ));
        }
    }
    let object_classes: Vec<&str> = plan
        .bindings
        .iter()
        .filter_map(Binding::class_name)
        .collect();
    for container in &plan.containers {
        if !object_classes.contains(&container.class_name.as_str()) {
            return Err(invalid(
                "containers",
                &format!(
                    "the container class {} has no Durable Object binding",
                    container.class_name
                ),
            ));
        }
    }
    for class_name in &object_classes {
        let migrated = plan
            .migrations
            .iter()
            .any(|migration| migration.new_sqlite_classes.iter().any(|c| c == class_name));
        if !migrated {
            return Err(invalid(
                "migrations",
                &format!("the Durable Object class {class_name} has no migration"),
            ));
        }
    }
    Ok(())
}

/// Validates a batch of plans that one deploy applies.
///
/// One Worker name has one main module. Two plans for the same Worker name
/// that name different main modules are refused, so a deploy cannot switch the
/// Worker module under a running deployment.
///
/// # Errors
/// [`DeployError::InvalidPlan`] for the first plan or pair that breaks a rule.
pub fn validate_plans(plans: &[DeploymentPlan]) -> Result<(), DeployError> {
    for (index, plan) in plans.iter().enumerate() {
        validate(plan)?;
        for other in &plans[..index] {
            if other.worker.name == plan.worker.name && other.worker.main != plan.worker.main {
                return Err(invalid(
                    "worker.main",
                    &format!(
                        "the Worker {} has two main modules: {} and {}",
                        plan.worker.name,
                        other.worker.main.display(),
                        plan.worker.main.display()
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// One invalid plan.
fn invalid(field: &str, reason: &str) -> DeployError {
    DeployError::InvalidPlan {
        field: field.to_string(),
        reason: reason.to_string(),
    }
}
