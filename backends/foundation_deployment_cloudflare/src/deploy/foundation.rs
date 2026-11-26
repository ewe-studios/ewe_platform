//! The primary deploy path: the Rust adapter.
//!
//! The adapter applies one [`DeploymentPlan`] in nine steps. A failure before
//! the traffic switch (step 8) leaves the previous Worker live.

use std::path::Path;

use foundation_context::Ctx;

use crate::deploy::host::{require, CommandSpec};
use crate::deploy::plan::{
    validate, Binding, ContainerDeployment, ContainerImage, DeploymentId, DeploymentOutcome,
    DeploymentPlan, DeploymentStatus, DoMigration, WorkerDeployment,
};

use crate::deploy::{deployed_unix_ms, host_from, CloudflareDeployer, DeployError, DeployFuture};

/// The primary deploy adapter.
///
/// It reads the [`crate::deploy::CloudflareDeployHost`] from the `Ctx`: the API
/// client, the image builder, the command runner and the secret store. A
/// missing entry is [`DeployError::HostNotConfigured`].
#[derive(Clone, Copy, Debug, Default)]
pub struct FoundationCloudflareDeployer;

impl FoundationCloudflareDeployer {
    /// The adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Steps 1 to 9 of feature 08, section 6.8.
    async fn deploy_steps(
        ctx: Ctx,
        plan: DeploymentPlan,
    ) -> Result<DeploymentOutcome, DeployError> {
        // Step 1: validate the plan. Nothing reaches Cloudflare on a failure.
        validate(&plan)?;
        let host = host_from(&ctx)?;
        let api = require(host.api.clone(), "api")?;
        let images = require(host.images.clone(), "images")?;
        let commands = require(host.commands.clone(), "commands")?;
        let secrets = require(host.secrets.clone(), "secrets")?;

        // Step 2: build the Worker, or use the wasm module that the plan names.
        build_worker(&*commands, &plan.worker).await?;

        // Step 3: build and push each container image.
        let mut container_tags = Vec::with_capacity(plan.containers.len());
        for container in &plan.containers {
            let tag = container_tag(&plan.worker.name, container);
            if let ContainerImage::Dockerfile(path) = &container.image {
                images.build(path, &tag).await?;
                images.push(&tag).await?;
            }
            container_tags.push(tag);
        }

        // Step 4: create or update the container application and its class.
        for (container, tag) in plan.containers.iter().zip(&container_tags) {
            api.ensure_container(&plan.worker.name, container, tag)
                .await?;
        }

        // Step 5: upload the new Worker version. It is not live yet.
        let version = api
            .upload_version(&plan.worker, &plan.bindings, &plan.migrations)
            .await?;

        // Step 6: set each secret on the new version. The value is never in the
        // plan and never in a log.
        for secret in &plan.secrets {
            let value = secrets.get(&secret.name, &secret.source).await?;
            api.put_secret(&plan.worker.name, &version, &secret.name, &value)
                .await?;
        }

        // Step 7: set the routes.
        for route in &plan.routes {
            api.set_route(&plan.worker.name, route).await?;
        }

        // Step 8: promote the version. This is the traffic switch.
        let promotion = api.promote(&plan.worker.name, &version).await?;

        // Step 9: the outcome.
        Ok(DeploymentOutcome {
            id: promotion.deployment_id,
            worker_url: promotion.worker_url,
            container_tags,
            deployed_unix_ms: deployed_unix_ms(&ctx),
        })
    }
}

impl CloudflareDeployer for FoundationCloudflareDeployer {
    fn deploy(
        &self,
        ctx: Ctx,
        plan: DeploymentPlan,
    ) -> DeployFuture<'_, Result<DeploymentOutcome, DeployError>> {
        Box::pin(Self::deploy_steps(ctx, plan))
    }

    fn status(
        &self,
        ctx: Ctx,
        id: &DeploymentId,
    ) -> DeployFuture<'_, Result<DeploymentStatus, DeployError>> {
        let id = id.clone();
        Box::pin(async move {
            let host = host_from(&ctx)?;
            let api = require(host.api.clone(), "api")?;
            api.status(&id).await
        })
    }

    fn rollback(&self, ctx: Ctx, id: &DeploymentId) -> DeployFuture<'_, Result<(), DeployError>> {
        let id = id.clone();
        Box::pin(async move {
            let host = host_from(&ctx)?;
            let api = require(host.api.clone(), "api")?;
            api.rollback(&id).await
        })
    }
}

/// The image tag of one container: the Worker name and the container name.
#[must_use]
pub fn container_tag(worker_name: &str, container: &ContainerDeployment) -> String {
    format!("{worker_name}-{}", container.name)
}

/// Builds the Worker, or accepts the wasm module that the plan names.
///
/// A `main` that ends in `.wasm` is a module that another build made. Every
/// other `main` comes from `worker-build --release` in the Worker directory.
async fn build_worker(
    commands: &dyn crate::deploy::CommandRunner,
    worker: &WorkerDeployment,
) -> Result<(), DeployError> {
    if is_wasm_module(&worker.main) {
        if !worker.main.is_file() {
            return Err(DeployError::Build {
                step: "worker module".to_string(),
                code: None,
                message: format!("the wasm module {} is absent", worker.main.display()),
            });
        }
        return Ok(());
    }
    let cwd = worker_dir(&worker.main);
    let output = commands
        .run(CommandSpec {
            program: "worker-build".to_string(),
            args: vec!["--release".to_string()],
            cwd: Some(cwd),
            stdin: None,
        })
        .await
        .map_err(|error| DeployError::Build {
            step: "worker-build".to_string(),
            code: None,
            message: error.to_string(),
        })?;
    if output.code != 0 {
        return Err(DeployError::Build {
            step: "worker-build".to_string(),
            code: Some(output.code),
            message: bounded(&output.stderr, &output.stdout),
        });
    }
    if !worker.main.is_file() {
        return Err(DeployError::Build {
            step: "worker-build".to_string(),
            code: Some(0),
            message: format!("the build left no main module at {}", worker.main.display()),
        });
    }
    Ok(())
}

/// Whether `main` is a WebAssembly module.
fn is_wasm_module(main: &Path) -> bool {
    main.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("wasm"))
}

/// The Worker directory of one `main`.
///
/// The `worker-build` output is `build/index.js`, so the directory is two
/// levels above it. Every other module sits in the Worker directory itself.
fn worker_dir(main: &Path) -> std::path::PathBuf {
    let parent = main.parent().unwrap_or_else(|| Path::new("."));
    if parent.file_name().and_then(std::ffi::OsStr::to_str) == Some("build") {
        parent.parent().unwrap_or(parent).to_path_buf()
    } else {
        parent.to_path_buf()
    }
}

/// The standard error of one command, or its standard output when the error is
/// empty, cut to 64 KiB.
fn bounded(stderr: &str, stdout: &str) -> String {
    const LIMIT: usize = 64 * 1024;
    let text = if stderr.trim().is_empty() {
        stdout
    } else {
        stderr
    };
    let mut end = text.len().min(LIMIT);
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

/// The names that one plan gives to the deploy, in the order that both adapters
/// read them.
///
/// A test compares this shape for the plan that each adapter received.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanShape {
    /// The Worker name.
    pub worker: String,
    /// One entry for each binding.
    pub bindings: Vec<String>,
    /// One entry for each migration tag.
    pub migrations: Vec<String>,
    /// One entry for each container class.
    pub containers: Vec<String>,
    /// One entry for each route pattern.
    pub routes: Vec<String>,
}

/// The shape of one plan.
#[must_use]
pub fn plan_shape(plan: &DeploymentPlan) -> PlanShape {
    PlanShape {
        worker: plan.worker.name.clone(),
        bindings: plan
            .bindings
            .iter()
            .map(|binding| match binding {
                Binding::DurableObject { name, class_name } => format!("DO {name} {class_name}"),
                other => other.name().to_string(),
            })
            .collect(),
        migrations: plan
            .migrations
            .iter()
            .map(|DoMigration { tag, .. }| tag.clone())
            .collect(),
        containers: plan
            .containers
            .iter()
            .map(|container| container.class_name.clone())
            .collect(),
        routes: plan
            .routes
            .iter()
            .map(|route| route.pattern.clone())
            .collect(),
    }
}
