//! The fallback deploy path: the `wrangler` adapter.
//!
//! The adapter writes the plan as a temporary `wrangler.toml`, copies the
//! Worker directory, runs `npx wrangler deploy` through an injected command
//! runner, then sets each secret. The command line interface (CLI) uploads the
//! Worker as one change, so a failed command leaves the previous Worker live.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use foundation_context::Ctx;

use crate::deploy::foundation::container_tag;
use crate::deploy::host::{require, CommandSpec};
use crate::deploy::plan::{
    validate, Binding, ContainerImage, DeploymentId, DeploymentOutcome, DeploymentPlan,
    DeploymentStatus, SecretSource,
};
use crate::deploy::{deployed_unix_ms, host_from, CloudflareDeployer, DeployError, DeployFuture};

/// The supported fallback deploy adapter.
///
/// `worker_name` goes to the `--name` flag of `status` and `rollback`. When it
/// is absent, `wrangler` reads the `wrangler.toml` of the working directory.
#[derive(Clone, Debug, Default)]
pub struct WranglerCloudflareDeployer {
    /// The Worker name of the status and rollback commands.
    pub worker_name: Option<String>,
}

impl WranglerCloudflareDeployer {
    /// The adapter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The adapter for one named Worker.
    #[must_use]
    pub fn for_worker(name: impl Into<String>) -> Self {
        Self {
            worker_name: Some(name.into()),
        }
    }

    /// Steps 1 to 5 of feature 08, section 6.8.
    async fn deploy_steps(
        ctx: Ctx,
        plan: DeploymentPlan,
    ) -> Result<DeploymentOutcome, DeployError> {
        // Step 1: validate the plan with the same function as the foundation
        // adapter.
        validate(&plan)?;
        let host = host_from(&ctx)?;
        let commands = require(host.commands.clone(), "commands")?;
        let secrets = require(host.secrets.clone(), "secrets")?;

        // Step 2: write the plan as a temporary `wrangler.toml` and copy the
        // Worker directory.
        let directory = temporary_directory(&ctx);
        prepare_directory(&plan, &directory)?;
        fs::write(directory.join("wrangler.toml"), wrangler_toml(&plan)).map_err(|error| {
            DeployError::Io {
                op: "write wrangler.toml".to_string(),
                message: error.to_string(),
            }
        })?;

        // Step 3: run the deploy. The command builds and pushes the Worker and
        // the image.
        let output = commands
            .run(CommandSpec {
                program: "npx".to_string(),
                args: vec!["wrangler".to_string(), "deploy".to_string()],
                cwd: Some(directory.clone()),
                stdin: None,
            })
            .await?;
        if output.code != 0 {
            return Err(DeployError::Build {
                step: "wrangler deploy".to_string(),
                code: Some(output.code),
                message: bounded(&output.stderr, &output.stdout),
            });
        }
        let worker_url =
            worker_url_from_output(&output.stdout).ok_or_else(|| DeployError::Api {
                status: 0,
                message: "the wrangler output named no Worker URL".to_string(),
            })?;

        // Step 4: set each secret. The value goes to the standard input and is
        // never in the plan or a log.
        for secret in &plan.secrets {
            let value = secrets.get(&secret.name, &secret.source).await?;
            let (args, stdin) = match &secret.source {
                SecretSource::WorkerSecret => (
                    vec![
                        "wrangler".to_string(),
                        "secret".to_string(),
                        "put".to_string(),
                        secret.name.clone(),
                    ],
                    value,
                ),
                SecretSource::SecretsStore { store } => (
                    vec![
                        "wrangler".to_string(),
                        "secrets-store".to_string(),
                        "secret".to_string(),
                        "create".to_string(),
                        store.clone(),
                        "--name".to_string(),
                        secret.name.clone(),
                        "--scopes".to_string(),
                        "workers".to_string(),
                    ],
                    value,
                ),
            };
            let output = commands
                .run(CommandSpec {
                    program: "npx".to_string(),
                    args,
                    cwd: Some(directory.clone()),
                    stdin: Some(stdin),
                })
                .await?;
            if output.code != 0 {
                return Err(DeployError::Api {
                    status: 0,
                    message: format!(
                        "the secret {} was not set: {}",
                        secret.name,
                        bounded(&output.stderr, &output.stdout)
                    ),
                });
            }
        }

        // Step 5: the same outcome shape as the foundation adapter.
        let container_tags = plan
            .containers
            .iter()
            .map(|container| container_tag(&plan.worker.name, container))
            .collect();
        Ok(DeploymentOutcome {
            id: DeploymentId::generate(&ctx),
            worker_url,
            container_tags,
            deployed_unix_ms: deployed_unix_ms(&ctx),
        })
    }
}

impl CloudflareDeployer for WranglerCloudflareDeployer {
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
        _id: &DeploymentId,
    ) -> DeployFuture<'_, Result<DeploymentStatus, DeployError>> {
        let worker_name = self.worker_name.clone();
        Box::pin(async move {
            let host = host_from(&ctx)?;
            let commands = require(host.commands.clone(), "commands")?;
            let mut args = vec![
                "wrangler".to_string(),
                "deployments".to_string(),
                "status".to_string(),
                "--json".to_string(),
            ];
            if let Some(name) = &worker_name {
                args.push("--name".to_string());
                args.push(name.clone());
            }
            let output = commands
                .run(CommandSpec {
                    program: "npx".to_string(),
                    args,
                    cwd: None,
                    stdin: None,
                })
                .await?;
            if output.code != 0 {
                return Err(DeployError::Api {
                    status: 0,
                    message: bounded(&output.stderr, &output.stdout),
                });
            }
            Ok(status_from_output(&output.stdout))
        })
    }

    fn rollback(&self, ctx: Ctx, id: &DeploymentId) -> DeployFuture<'_, Result<(), DeployError>> {
        let worker_name = self.worker_name.clone();
        let version = id.as_str().to_string();
        Box::pin(async move {
            let host = host_from(&ctx)?;
            let commands = require(host.commands.clone(), "commands")?;
            let mut args = vec![
                "wrangler".to_string(),
                "rollback".to_string(),
                version,
                "--yes".to_string(),
            ];
            if let Some(name) = &worker_name {
                args.push("--name".to_string());
                args.push(name.clone());
            }
            let output = commands
                .run(CommandSpec {
                    program: "npx".to_string(),
                    args,
                    cwd: None,
                    stdin: None,
                })
                .await?;
            if output.code != 0 {
                return Err(DeployError::Api {
                    status: 0,
                    message: bounded(&output.stderr, &output.stdout),
                });
            }
            Ok(())
        })
    }
}

/// The deployment state of the text of `wrangler deployments status --json`.
///
/// A successful status with no state word means that the Worker is live.
fn status_from_output(output: &str) -> DeploymentStatus {
    let lower = output.to_lowercase();
    if lower.contains("rolled") {
        DeploymentStatus::RolledBack
    } else if lower.contains("failed") {
        DeploymentStatus::Failed
    } else if lower.contains("queued") {
        DeploymentStatus::Queued
    } else if lower.contains("building") {
        DeploymentStatus::Building
    } else if lower.contains("uploading") {
        DeploymentStatus::Uploading
    } else {
        DeploymentStatus::Live
    }
}

/// The text of the `wrangler.toml` for one plan.
///
/// It is a view of the plan for the manual path. The plan is the source of
/// truth. The `[build]` command is present only when the plan names a source
/// module, not a wasm module.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "one text builder for the whole plan is clearer than six helpers"
)]
pub fn wrangler_toml(plan: &DeploymentPlan) -> String {
    let mut toml = String::new();
    let _ = writeln!(toml, "name = {}", toml_string(&plan.worker.name));
    let _ = writeln!(
        toml,
        "main = {}",
        toml_string(&plan.worker.main.to_string_lossy())
    );
    let _ = writeln!(
        toml,
        "compatibility_date = {}",
        toml_string(&plan.worker.compatibility_date)
    );
    if !is_wasm_module(&plan.worker.main) {
        toml.push_str("\n[build]\ncommand = \"worker-build --release\"\n");
    }
    if !plan.routes.is_empty() {
        let routes: Vec<String> = plan
            .routes
            .iter()
            .map(|route| match &route.zone {
                Some(zone) => format!(
                    "{{ pattern = {}, zone_name = {} }}",
                    toml_string(&route.pattern),
                    toml_string(zone)
                ),
                None => format!("{{ pattern = {} }}", toml_string(&route.pattern)),
            })
            .collect();
        let _ = writeln!(toml, "routes = [{}]", routes.join(", "));
    }

    let mut vars: Vec<(String, String)> = Vec::new();
    for binding in &plan.bindings {
        match binding {
            Binding::DurableObject { name, class_name } => {
                let _ = write!(
                    toml,
                    "\n[[durable_objects.bindings]]\nname = {}\nclass_name = {}\n",
                    toml_string(name),
                    toml_string(class_name)
                );
            }
            Binding::D1 { name, database_id } => {
                let _ = write!(
                    toml,
                    "\n[[d1_databases]]\nbinding = {}\ndatabase_id = {}\n",
                    toml_string(name),
                    toml_string(database_id)
                );
            }
            Binding::Kv { name, namespace_id } => {
                let _ = write!(
                    toml,
                    "\n[[kv_namespaces]]\nbinding = {}\nid = {}\n",
                    toml_string(name),
                    toml_string(namespace_id)
                );
            }
            Binding::R2 { name, bucket_name } => {
                let _ = write!(
                    toml,
                    "\n[[r2_buckets]]\nbinding = {}\nbucket_name = {}\n",
                    toml_string(name),
                    toml_string(bucket_name)
                );
            }
            Binding::Ai { name } => {
                let _ = write!(toml, "\n[ai]\nbinding = {}\n", toml_string(name));
            }
            Binding::Service { name, service } => {
                let _ = write!(
                    toml,
                    "\n[[services]]\nbinding = {}\nservice = {}\n",
                    toml_string(name),
                    toml_string(service)
                );
            }
            Binding::PlainText { name, value } => {
                vars.push((name.clone(), value.clone()));
            }
        }
    }
    if !vars.is_empty() {
        toml.push_str("\n[vars]\n");
        for (name, value) in vars {
            let _ = writeln!(toml, "{} = {}", toml_string(&name), toml_string(&value));
        }
    }

    for migration in &plan.migrations {
        let classes: Vec<String> = migration
            .new_sqlite_classes
            .iter()
            .map(|class| toml_string(class))
            .collect();
        let _ = write!(
            toml,
            "\n[[migrations]]\ntag = {}\nnew_sqlite_classes = [{}]\n",
            toml_string(&migration.tag),
            classes.join(", ")
        );
    }

    for container in &plan.containers {
        let image = match &container.image {
            ContainerImage::Dockerfile(path) => path.to_string_lossy().into_owned(),
            ContainerImage::Registry { tag, .. } => tag.clone(),
        };
        let _ = write!(
            toml,
            "\n[[containers]]\nclass_name = {}\nname = {}\nimage = {}\nmax_instances = {}\ninstance_type = {}\n",
            toml_string(&container.class_name),
            toml_string(&container.name),
            toml_string(&image),
            container.max_instances,
            toml_string(&container.instance_type)
        );
    }
    toml
}

/// The first Worker URL in the output of `wrangler deploy`.
#[must_use]
pub fn worker_url_from_output(output: &str) -> Option<String> {
    let candidates = || {
        output
            .split_whitespace()
            .map(|token| token.trim_matches(|c: char| ",.;()[]".contains(c)))
    };
    candidates()
        .find(|token| token.starts_with("https://") && token.contains("workers.dev"))
        .or_else(|| candidates().find(|token| token.starts_with("https://")))
        .map(str::to_string)
}

/// A unique temporary directory for one deploy.
fn temporary_directory(ctx: &Ctx) -> PathBuf {
    std::env::temp_dir().join(format!(
        "ewe-sandbox-deploy-{}",
        DeploymentId::generate(ctx)
    ))
}

/// Copies the Worker directory into `directory`.
///
/// A wasm `main` is copied alone. Every other `main` is the `worker-build`
/// output, so the whole source directory goes to the temporary directory and
/// the `[build]` command makes the output again.
fn prepare_directory(plan: &DeploymentPlan, directory: &Path) -> Result<(), DeployError> {
    fs::create_dir_all(directory).map_err(|error| DeployError::Io {
        op: "create the temporary directory".to_string(),
        message: error.to_string(),
    })?;
    let main = &plan.worker.main;
    if is_wasm_module(main) {
        let destination = directory.join("module.wasm");
        fs::copy(main, &destination).map_err(|error| DeployError::Io {
            op: "copy the wasm module".to_string(),
            message: error.to_string(),
        })?;
        return Ok(());
    }
    let source = worker_dir(main);
    if !source.is_dir() {
        return Err(DeployError::Io {
            op: "copy the Worker directory".to_string(),
            message: format!("the Worker directory {} is absent", source.display()),
        });
    }
    copy_tree(&source, directory)
}

/// Copies one directory tree, without the build output and the Git directory.
fn copy_tree(from: &Path, to: &Path) -> Result<(), DeployError> {
    fs::create_dir_all(to).map_err(|error| DeployError::Io {
        op: "create a copied directory".to_string(),
        message: error.to_string(),
    })?;
    let entries = fs::read_dir(from).map_err(|error| DeployError::Io {
        op: "read the Worker directory".to_string(),
        message: error.to_string(),
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| DeployError::Io {
            op: "read the Worker directory".to_string(),
            message: error.to_string(),
        })?;
        let name = entry.file_name();
        if matches!(
            name.to_str(),
            Some("target" | "build" | ".git" | "node_modules")
        ) {
            continue;
        }
        let path = entry.path();
        let destination = to.join(&name);
        let file_type = entry.file_type().map_err(|error| DeployError::Io {
            op: "read a Worker entry".to_string(),
            message: error.to_string(),
        })?;
        if file_type.is_dir() {
            copy_tree(&path, &destination)?;
        } else {
            fs::copy(&path, &destination).map_err(|error| DeployError::Io {
                op: "copy a Worker file".to_string(),
                message: error.to_string(),
            })?;
        }
    }
    Ok(())
}

/// The Worker directory of one `main`, as in the foundation adapter.
fn worker_dir(main: &Path) -> PathBuf {
    let parent = main.parent().unwrap_or_else(|| Path::new("."));
    if parent.file_name().and_then(std::ffi::OsStr::to_str) == Some("build") {
        parent.parent().unwrap_or(parent).to_path_buf()
    } else {
        parent.to_path_buf()
    }
}

/// Whether `main` is a WebAssembly module.
fn is_wasm_module(main: &Path) -> bool {
    main.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("wasm"))
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

/// One string in TOML form.
fn toml_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}
