//! The deploy-plan tests of feature 08, section 7.
//!
//! Every test uses fakes: no network call and no container engine run.

use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use foundation_auth::server_credential::Secret;
use foundation_context::Ctx;
use foundation_core::valtron::{block_on_future, SimulatedClock};
use foundation_deployment_cloudflare::deploy::foundation::plan_shape;
use foundation_deployment_cloudflare::{
    default_deployer, validate, validate_plans, Binding, CloudflareDeployHost, CloudflareDeployer,
    CommandOutput, CommandRunner, CommandSpec, ContainerImage, DeployError, DeploymentApi,
    DeploymentId, DeploymentOutcome, DeploymentPlan, DeploymentStatus, DoMigration,
    FoundationCloudflareDeployer, ImageBuilder, Route, SecretRef, SecretSource, SecretStore,
    WorkerDeployment, WranglerCloudflareDeployer,
};

/// A unique temporary directory for one test.
fn temporary_directory(name: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "ewe-deploy-test-{}-{}-{name}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(&path).expect("the test directory");
    path
}

/// Runs one future on a valtron pool that this test owns.
fn run<F>(future: F) -> F::Output
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let _pool = foundation_core::valtron::initialize_pool(7, Some(3));
    block_on_future(future)
}

/// A `Ctx` that carries `host`.
fn context(host: CloudflareDeployHost) -> Ctx {
    let ctx = Ctx::fixed(Arc::new(SimulatedClock::new(Duration::ZERO)));
    ctx.store(host);
    ctx
}

/// The sandbox plan, with real files under `directory`.
fn sandbox_plan(directory: &Path) -> DeploymentPlan {
    let mut plan = DeploymentPlan::sandbox_worker();
    plan.worker.main = directory.join("build/index.js");
    plan.containers[0].image = ContainerImage::Dockerfile(directory.join("Dockerfile"));
    plan.routes = vec![Route {
        pattern: "sandbox.example.com/*".to_string(),
        zone: Some("example.com".to_string()),
    }];
    fs::create_dir_all(directory.join("build")).expect("the build directory");
    fs::write(&plan.worker.main, b"export default {}").expect("the main module");
    fs::write(directory.join("Dockerfile"), b"FROM alpine:3.22\n").expect("the Dockerfile");
    plan
}

/// One API call that a fake recorded.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ApiCall {
    EnsureContainer {
        worker: String,
        class_name: String,
        tag: String,
    },
    UploadVersion {
        worker: String,
        bindings: Vec<(String, Option<String>)>,
        migrations: Vec<String>,
    },
    PutSecret {
        worker: String,
        version: String,
        name: String,
        value: String,
    },
    SetRoute {
        worker: String,
        pattern: String,
    },
    Promote {
        worker: String,
        version: String,
    },
    Status,
    Rollback,
}

/// The step at which the fake API fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    EnsureContainer,
    UploadVersion,
    PutSecret,
    SetRoute,
    Promote,
}

/// A fake Cloudflare API.
#[derive(Clone, Default)]
struct FakeApi {
    calls: Arc<Mutex<Vec<ApiCall>>>,
    fail_at: Arc<Mutex<Option<Step>>>,
}

impl FakeApi {
    fn calls(&self) -> Vec<ApiCall> {
        self.calls.lock().expect("the call log").clone()
    }

    fn fail_at(&self, step: Step) {
        *self.fail_at.lock().expect("the failure cell") = Some(step);
    }

    fn maybe_fail(&self, step: Step) -> Result<(), DeployError> {
        if *self.fail_at.lock().expect("the failure cell") == Some(step) {
            Err(DeployError::Api {
                status: 500,
                message: format!("the fake API failed at {step:?}"),
            })
        } else {
            Ok(())
        }
    }
}

impl DeploymentApi for FakeApi {
    fn ensure_container(
        &self,
        worker_name: &str,
        container: &foundation_deployment_cloudflare::ContainerDeployment,
        tag: &str,
    ) -> foundation_deployment_cloudflare::deploy::DeployFuture<'_, Result<(), DeployError>> {
        let call = ApiCall::EnsureContainer {
            worker: worker_name.to_string(),
            class_name: container.class_name.clone(),
            tag: tag.to_string(),
        };
        self.calls.lock().expect("the call log").push(call);
        let failure = self.maybe_fail(Step::EnsureContainer);
        Box::pin(async move { failure })
    }

    fn upload_version(
        &self,
        worker: &WorkerDeployment,
        bindings: &[Binding],
        migrations: &[DoMigration],
    ) -> foundation_deployment_cloudflare::deploy::DeployFuture<'_, Result<String, DeployError>>
    {
        let call = ApiCall::UploadVersion {
            worker: worker.name.clone(),
            bindings: bindings
                .iter()
                .map(|binding| {
                    (
                        binding.name().to_string(),
                        binding.class_name().map(str::to_string),
                    )
                })
                .collect(),
            migrations: migrations
                .iter()
                .map(|migration| migration.tag.clone())
                .collect(),
        };
        self.calls.lock().expect("the call log").push(call);
        let failure = self.maybe_fail(Step::UploadVersion);
        Box::pin(async move {
            failure?;
            Ok("version-1".to_string())
        })
    }

    fn put_secret(
        &self,
        worker_name: &str,
        version: &str,
        name: &str,
        value: &Secret,
    ) -> foundation_deployment_cloudflare::deploy::DeployFuture<'_, Result<(), DeployError>> {
        let call = ApiCall::PutSecret {
            worker: worker_name.to_string(),
            version: version.to_string(),
            name: name.to_string(),
            value: value.expose().to_string(),
        };
        self.calls.lock().expect("the call log").push(call);
        let failure = self.maybe_fail(Step::PutSecret);
        Box::pin(async move { failure })
    }

    fn set_route(
        &self,
        worker_name: &str,
        route: &Route,
    ) -> foundation_deployment_cloudflare::deploy::DeployFuture<'_, Result<(), DeployError>> {
        let call = ApiCall::SetRoute {
            worker: worker_name.to_string(),
            pattern: route.pattern.clone(),
        };
        self.calls.lock().expect("the call log").push(call);
        let failure = self.maybe_fail(Step::SetRoute);
        Box::pin(async move { failure })
    }

    fn promote(
        &self,
        worker_name: &str,
        version: &str,
    ) -> foundation_deployment_cloudflare::deploy::DeployFuture<'_, Result<String, DeployError>>
    {
        let call = ApiCall::Promote {
            worker: worker_name.to_string(),
            version: version.to_string(),
        };
        self.calls.lock().expect("the call log").push(call);
        let failure = self.maybe_fail(Step::Promote);
        Box::pin(async move {
            failure?;
            Ok("https://ewe-sandbox.example.workers.dev".to_string())
        })
    }

    fn status(
        &self,
        _id: &DeploymentId,
    ) -> foundation_deployment_cloudflare::deploy::DeployFuture<
        '_,
        Result<DeploymentStatus, DeployError>,
    > {
        self.calls
            .lock()
            .expect("the call log")
            .push(ApiCall::Status);
        Box::pin(async { Ok(DeploymentStatus::Live) })
    }

    fn rollback(
        &self,
        _id: &DeploymentId,
    ) -> foundation_deployment_cloudflare::deploy::DeployFuture<'_, Result<(), DeployError>> {
        self.calls
            .lock()
            .expect("the call log")
            .push(ApiCall::Rollback);
        Box::pin(async { Ok(()) })
    }
}

/// A fake image builder.
#[derive(Clone, Default)]
struct FakeImages {
    calls: Arc<Mutex<Vec<String>>>,
}

impl ImageBuilder for FakeImages {
    fn build(
        &self,
        dockerfile: &Path,
        tag: &str,
    ) -> foundation_deployment_cloudflare::deploy::DeployFuture<'_, Result<(), DeployError>> {
        self.calls
            .lock()
            .expect("the image log")
            .push(format!("build {} {tag}", dockerfile.display()));
        Box::pin(async { Ok(()) })
    }

    fn push(
        &self,
        tag: &str,
    ) -> foundation_deployment_cloudflare::deploy::DeployFuture<'_, Result<(), DeployError>> {
        self.calls
            .lock()
            .expect("the image log")
            .push(format!("push {tag}"));
        Box::pin(async { Ok(()) })
    }
}

/// One command that the fake ran.
#[derive(Clone, Debug, PartialEq, Eq)]
struct CommandRecord {
    program: String,
    args: Vec<String>,
    cwd: Option<PathBuf>,
    stdin: Option<String>,
    toml: Option<String>,
}

/// A fake command runner.
#[derive(Clone, Default)]
struct FakeCommands {
    calls: Arc<Mutex<Vec<CommandRecord>>>,
    stdout: Arc<Mutex<String>>,
    status_stdout: Arc<Mutex<String>>,
}

impl FakeCommands {
    fn with_stdout(stdout: &str) -> Self {
        let commands = Self::default();
        *commands.stdout.lock().expect("the stdout cell") = stdout.to_string();
        commands
    }

    fn with_status_stdout(self, stdout: &str) -> Self {
        *self.status_stdout.lock().expect("the status stdout cell") = stdout.to_string();
        self
    }

    fn calls(&self) -> Vec<CommandRecord> {
        self.calls.lock().expect("the command log").clone()
    }
}

impl CommandRunner for FakeCommands {
    fn run(
        &self,
        spec: CommandSpec,
    ) -> foundation_deployment_cloudflare::deploy::DeployFuture<
        '_,
        Result<CommandOutput, DeployError>,
    > {
        let toml = spec
            .cwd
            .as_ref()
            .and_then(|cwd| fs::read_to_string(cwd.join("wrangler.toml")).ok());
        let is_status = spec.args.iter().any(|arg| arg == "status");
        self.calls
            .lock()
            .expect("the command log")
            .push(CommandRecord {
                program: spec.program,
                args: spec.args,
                cwd: spec.cwd,
                stdin: spec.stdin.as_ref().map(|value| value.expose().to_string()),
                toml,
            });
        let stdout = if is_status {
            self.status_stdout
                .lock()
                .expect("the status stdout cell")
                .clone()
        } else {
            self.stdout.lock().expect("the stdout cell").clone()
        };
        Box::pin(async move {
            Ok(CommandOutput {
                code: 0,
                stdout,
                stderr: String::new(),
            })
        })
    }
}

/// A fake secret store.
#[derive(Clone)]
struct FakeSecrets {
    calls: Arc<Mutex<Vec<(String, SecretSource)>>>,
}

impl Default for FakeSecrets {
    fn default() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl SecretStore for FakeSecrets {
    fn get(
        &self,
        name: &str,
        source: &SecretSource,
    ) -> foundation_deployment_cloudflare::deploy::DeployFuture<'_, Result<Secret, DeployError>>
    {
        self.calls
            .lock()
            .expect("the secret log")
            .push((name.to_string(), source.clone()));
        Box::pin(async { Ok(Secret::new("hunter2")) })
    }
}

/// A host with every fake.
struct Fakes {
    api: FakeApi,
    images: FakeImages,
    commands: FakeCommands,
    secrets: FakeSecrets,
}

impl Fakes {
    fn new() -> Self {
        Self::with_stdout("")
    }

    fn with_stdout(stdout: &str) -> Self {
        Self {
            api: FakeApi::default(),
            images: FakeImages::default(),
            commands: FakeCommands::with_stdout(stdout),
            secrets: FakeSecrets::default(),
        }
    }

    fn host(&self) -> CloudflareDeployHost {
        CloudflareDeployHost::new()
            .with_api(Arc::new(self.api.clone()))
            .with_images(Arc::new(self.images.clone()))
            .with_commands(Arc::new(self.commands.clone()))
            .with_secrets(Arc::new(self.secrets.clone()))
    }
}

/// Runs one deploy on the foundation path.
fn foundation_deploy(ctx: Ctx, plan: DeploymentPlan) -> Result<DeploymentOutcome, DeployError> {
    run(async move {
        let deployer = FoundationCloudflareDeployer::new();
        deployer.deploy(ctx, plan).await
    })
}

/// Runs one deploy on the `wrangler` path.
fn wrangler_deploy(ctx: Ctx, plan: DeploymentPlan) -> Result<DeploymentOutcome, DeployError> {
    run(async move {
        let deployer = WranglerCloudflareDeployer::new();
        deployer.deploy(ctx, plan).await
    })
}

/// Both adapters receive the same plan and name the same parts (invariant I10).
#[test]
fn deploy_plan_parity() {
    let directory = temporary_directory("parity");
    let plan = sandbox_plan(&directory);
    let shape = plan_shape(&plan);

    let foundation = Fakes::new();
    let outcome = foundation_deploy(context(foundation.host()), plan.clone()).expect("the deploy");
    assert_eq!(
        outcome.worker_url,
        "https://ewe-sandbox.example.workers.dev"
    );

    let wrangler =
        Fakes::with_stdout("Deployed ewe-sandbox to https://ewe-sandbox.example.workers.dev");
    let wrangler_outcome =
        wrangler_deploy(context(wrangler.host()), plan.clone()).expect("the deploy");
    assert_eq!(wrangler_outcome.worker_url, outcome.worker_url);

    let calls = foundation.api.calls();
    let upload = calls
        .iter()
        .find_map(|call| match call {
            ApiCall::UploadVersion {
                worker,
                bindings,
                migrations,
            } => Some((worker.clone(), bindings.clone(), migrations.clone())),
            _ => None,
        })
        .expect("the version upload");
    assert_eq!(upload.0, shape.worker);
    assert_eq!(upload.2, shape.migrations);
    let names: Vec<String> = upload.1.iter().map(|(name, _)| name.clone()).collect();
    assert_eq!(
        names,
        vec!["SANDBOX", "SANDBOX_INDEX", "SLAVE", "AI"],
        "the bindings of the plan"
    );

    let recorded = wrangler.commands.calls();
    let deploy = recorded
        .iter()
        .find(|call| call.args == ["wrangler", "deploy"])
        .expect("the deploy command");
    let toml = deploy.toml.as_deref().expect("the written wrangler.toml");
    for part in [
        &format!("name = \"{}\"", shape.worker),
        "name = \"SANDBOX\"",
        "class_name = \"SandboxDo\"",
        "name = \"SLAVE\"",
        "class_name = \"ModelSlaveDo\"",
        "tag = \"v1\"",
        "pattern = \"sandbox.example.com/*\"",
    ] {
        assert!(toml.contains(part), "the wrangler.toml must hold {part:?}");
    }
}

/// A deploy with no adapter selected uses the foundation path (invariant I11).
#[test]
fn default_deployer_is_foundation() {
    let directory = temporary_directory("default");
    let plan = sandbox_plan(&directory);
    let fakes = Fakes::new();
    let ctx = context(fakes.host());
    run(async move { default_deployer().deploy(ctx, plan).await }).expect("the deploy");
    assert!(
        fakes
            .api
            .calls()
            .iter()
            .any(|call| matches!(call, ApiCall::EnsureContainer { .. })),
        "the foundation path reached the API"
    );
    assert!(
        !fakes
            .commands
            .calls()
            .iter()
            .any(|call| call.args == ["wrangler", "deploy"]),
        "the wrangler path did not run"
    );
}

/// The plan holds secret names and sources, never a value (invariant I12).
#[test]
fn deploy_plan_has_no_secret_value() {
    let plan = DeploymentPlan::sandbox_worker().with_open_router_key();
    let text = format!("{plan:?}");
    assert!(text.contains("SANDBOX_TOKEN"));
    assert!(text.contains("MODEL_API_KEY"));
    assert!(!text.contains("hunter2"));
    assert_eq!(plan.secrets.len(), 2);
}

/// A secret value reaches the API and never the plan (invariant I12).
#[test]
fn deploy_sets_secret_from_source() {
    let directory = temporary_directory("secret");
    let plan = sandbox_plan(&directory);
    let fakes = Fakes::new();
    foundation_deploy(context(fakes.host()), plan.clone()).expect("the deploy");
    let values: Vec<String> = fakes
        .api
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            ApiCall::PutSecret { name, value, .. } => {
                assert_eq!(name, "SANDBOX_TOKEN");
                Some(value)
            }
            _ => None,
        })
        .collect();
    assert_eq!(values, vec!["hunter2".to_string()]);
    assert!(!format!("{plan:?}").contains("hunter2"));
}

/// A failure at each step before the traffic switch leaves the old Worker live
/// (invariant I13).
#[test]
fn deploy_failure_keeps_previous() {
    for step in [
        Step::EnsureContainer,
        Step::UploadVersion,
        Step::PutSecret,
        Step::SetRoute,
        Step::Promote,
    ] {
        let directory = temporary_directory("failure");
        let plan = sandbox_plan(&directory);
        let fakes = Fakes::new();
        fakes.api.fail_at(step);
        let outcome = foundation_deploy(context(fakes.host()), plan);
        assert!(outcome.is_err(), "the deploy must fail at {step:?}");
        let promoted = fakes
            .api
            .calls()
            .iter()
            .filter(|call| matches!(call, ApiCall::Promote { .. }))
            .count();
        if step == Step::Promote {
            assert_eq!(promoted, 1, "the traffic switch was attempted");
        } else {
            assert_eq!(promoted, 0, "no traffic switch before {step:?} failed");
        }
    }
}

/// Rollback is idempotent (invariant I13).
#[test]
fn rollback_idempotent() {
    let fakes = Fakes::new();
    let ctx = context(fakes.host());
    let id = DeploymentId::new("deployment-1");
    run(async move {
        let deployer = FoundationCloudflareDeployer::new();
        deployer.rollback(ctx.clone(), &id).await?;
        deployer.rollback(ctx, &id).await
    })
    .expect("both rollbacks succeed");
    let rollbacks = fakes
        .api
        .calls()
        .iter()
        .filter(|call| matches!(call, ApiCall::Rollback))
        .count();
    assert_eq!(rollbacks, 2);
}

/// Plan validation refuses a bad name, a duplicate binding and a missing
/// migration.
#[test]
fn deploy_plan_invalid() {
    let mut empty_name = DeploymentPlan::sandbox_worker();
    empty_name.worker.name = String::new();
    match validate(&empty_name) {
        Err(DeployError::InvalidPlan { field, .. }) => assert_eq!(field, "worker.name"),
        other => panic!("expected InvalidPlan, got {other:?}"),
    }

    let mut duplicate = DeploymentPlan::sandbox_worker();
    duplicate.bindings.push(Binding::Ai {
        name: "AI".to_string(),
    });
    match validate(&duplicate) {
        Err(DeployError::InvalidPlan { field, .. }) => assert_eq!(field, "bindings"),
        other => panic!("expected InvalidPlan, got {other:?}"),
    }

    let mut missing_migration = DeploymentPlan::sandbox_worker();
    missing_migration.migrations.clear();
    match validate(&missing_migration) {
        Err(DeployError::InvalidPlan { field, .. }) => assert_eq!(field, "migrations"),
        other => panic!("expected InvalidPlan, got {other:?}"),
    }
}

/// The foundation adapter runs each step in order.
#[test]
fn foundation_deploy_mock() {
    let directory = temporary_directory("foundation");
    let plan = sandbox_plan(&directory);
    let fakes = Fakes::new();
    let outcome = foundation_deploy(context(fakes.host()), plan).expect("the deploy");

    let command = fakes
        .commands
        .calls()
        .into_iter()
        .find(|call| call.program == "worker-build")
        .expect("the Worker build");
    assert_eq!(command.args, ["--release"]);
    assert_eq!(command.cwd, Some(directory.clone()));

    assert_eq!(
        fakes.images.calls.lock().expect("the image log").as_slice(),
        [
            format!(
                "build {} ewe-sandbox-ewe-sandbox",
                directory.join("Dockerfile").display()
            ),
            "push ewe-sandbox-ewe-sandbox".to_string(),
        ]
    );

    let calls = fakes.api.calls();
    let positions: Vec<&ApiCall> = calls
        .iter()
        .filter(|call| {
            matches!(
                call,
                ApiCall::EnsureContainer { .. }
                    | ApiCall::UploadVersion { .. }
                    | ApiCall::PutSecret { .. }
                    | ApiCall::SetRoute { .. }
                    | ApiCall::Promote { .. }
            )
        })
        .collect();
    assert!(matches!(positions[0], ApiCall::EnsureContainer { .. }));
    assert!(matches!(positions[1], ApiCall::UploadVersion { .. }));
    assert!(matches!(positions[2], ApiCall::PutSecret { .. }));
    assert!(matches!(positions[3], ApiCall::SetRoute { .. }));
    assert!(matches!(positions[4], ApiCall::Promote { .. }));

    let secrets = fakes.secrets.calls.lock().expect("the secret log").clone();
    assert_eq!(secrets.len(), 1);
    assert_eq!(secrets[0].0, "SANDBOX_TOKEN");
    assert_eq!(outcome.container_tags, vec!["ewe-sandbox-ewe-sandbox"]);
    assert_ne!(outcome.id.as_str(), "");
}

/// The `wrangler` adapter writes the plan, runs the deploy and sets each
/// secret through the CLI.
#[test]
fn wrangler_deploy_command_lines() {
    let directory = temporary_directory("wrangler");
    let plan = sandbox_plan(&directory);
    let fakes = Fakes::with_stdout(
        "Uploaded ewe-sandbox\nDeployed to https://ewe-sandbox.example.workers.dev",
    );
    let main_line = format!("main = \"{}\"", plan.worker.main.display());
    let outcome = wrangler_deploy(context(fakes.host()), plan).expect("the deploy");

    let calls = fakes.commands.calls();
    let deploy = calls
        .iter()
        .find(|call| call.args == ["wrangler", "deploy"])
        .expect("the deploy command");
    assert_eq!(deploy.program, "npx");
    let toml = deploy.toml.as_deref().expect("the wrangler.toml");
    for part in [
        "name = \"ewe-sandbox\"",
        main_line.as_str(),
        "[build]",
        "command = \"worker-build --release\"",
        "[[durable_objects.bindings]]",
        "name = \"SANDBOX\"",
        "class_name = \"SandboxDo\"",
        "[ai]",
        "binding = \"AI\"",
        "[[migrations]]",
        "new_sqlite_classes = [\"SandboxDo\", \"SandboxIndexDo\", \"ModelSlaveDo\"]",
        "[[containers]]",
        "max_instances = 10",
        "instance_type = \"lite\"",
    ] {
        assert!(toml.contains(part), "the wrangler.toml must hold {part:?}");
    }

    let secret = calls
        .iter()
        .find(|call| call.args == ["wrangler", "secret", "put", "SANDBOX_TOKEN"])
        .expect("the secret command");
    assert_eq!(secret.program, "npx");
    assert_eq!(secret.stdin.as_deref(), Some("hunter2"));
    assert_eq!(
        outcome.worker_url,
        "https://ewe-sandbox.example.workers.dev"
    );
}

/// Two plans for one Worker name cannot name two main modules (#919).
#[test]
fn deploy_plans_with_two_mains() {
    let directory = temporary_directory("two-mains");
    let first = sandbox_plan(&directory);
    let mut second = first.clone();
    second.worker.main = directory.join("build/second.js");
    match validate_plans(&[first.clone(), second]) {
        Err(DeployError::InvalidPlan { field, .. }) => assert_eq!(field, "worker.main"),
        other => panic!("expected InvalidPlan, got {other:?}"),
    }
    assert_eq!(
        validate_plans(&[first.clone(), first]),
        Ok(()),
        "one main module is valid"
    );
}

/// The `wrangler` CLI cannot take a Secrets Store value from standard input
/// (#920).
#[test]
fn wrangler_secrets_store_is_reported() {
    let directory = temporary_directory("secrets-store");
    let mut plan = sandbox_plan(&directory);
    plan.secrets = vec![SecretRef {
        name: "SANDBOX_TOKEN".to_string(),
        source: SecretSource::SecretsStore {
            store: "store-1".to_string(),
        },
    }];
    let fakes = Fakes::with_stdout("https://ewe-sandbox.example.workers.dev");
    let error = wrangler_deploy(context(fakes.host()), plan).expect_err("the deploy stops");
    assert!(
        matches!(&error, DeployError::Unsupported { what } if what.contains("Secrets Store")),
        "the error names the limit: {error}"
    );
}

/// The `wrangler` adapter reads the status and rolls back with the CLI.
#[test]
fn wrangler_status_and_rollback() {
    let fakes = Fakes::with_stdout("");
    let commands = fakes
        .commands
        .clone()
        .with_status_stdout(r#"{"id":"version-42","status":"active"}"#);
    let host = CloudflareDeployHost::new().with_commands(Arc::new(commands));
    let ctx = context(host);
    let id = DeploymentId::new("version-42");

    let status = run({
        let ctx = ctx.clone();
        let id = id.clone();
        async move {
            let deployer = WranglerCloudflareDeployer::for_worker("ewe-sandbox");
            deployer.status(ctx, &id).await
        }
    })
    .expect("the status");
    assert_eq!(status, DeploymentStatus::Live);

    run({
        let ctx = ctx.clone();
        let id = id.clone();
        async move {
            let deployer = WranglerCloudflareDeployer::for_worker("ewe-sandbox");
            deployer.rollback(ctx.clone(), &id).await?;
            deployer.rollback(ctx, &id).await
        }
    })
    .expect("both rollbacks succeed");

    let calls = fakes.commands.calls();
    let status = calls
        .iter()
        .find(|call| call.args.iter().any(|arg| arg == "status"))
        .expect("the status command");
    assert_eq!(
        status.args,
        [
            "wrangler",
            "deployments",
            "status",
            "--json",
            "--name",
            "ewe-sandbox"
        ]
    );
    let rollbacks: Vec<&CommandRecord> = calls
        .iter()
        .filter(|call| call.args.get(1).is_some_and(|arg| arg == "rollback"))
        .collect();
    assert_eq!(rollbacks.len(), 2, "rollback is idempotent");
    assert_eq!(
        rollbacks[0].args,
        [
            "wrangler",
            "rollback",
            "version-42",
            "--yes",
            "--name",
            "ewe-sandbox"
        ]
    );
}

/// The `wrangler` adapter copies the Worker directory; the source directory
/// stays unchanged.
#[test]
fn wrangler_copies_the_worker() {
    let directory = temporary_directory("wrangler-copy");
    let plan = sandbox_plan(&directory);
    let fakes = Fakes::with_stdout("https://ewe-sandbox.example.workers.dev");
    wrangler_deploy(context(fakes.host()), plan).expect("the deploy");
    let deploy = fakes
        .commands
        .calls()
        .into_iter()
        .find(|call| call.args == ["wrangler", "deploy"])
        .expect("the deploy command");
    let cwd = deploy.cwd.expect("the temporary directory");
    assert_ne!(cwd, directory);
    assert!(cwd.join("wrangler.toml").is_file());
    assert!(cwd.join("Dockerfile").is_file());
    assert!(
        directory.join("Dockerfile").is_file(),
        "the source directory stays"
    );
}

/// A missing host entry is a typed error.
#[test]
fn deploy_without_a_host_is_typed() {
    let directory = temporary_directory("no-host");
    let plan = sandbox_plan(&directory);
    let ctx = Ctx::fixed(Arc::new(SimulatedClock::new(Duration::ZERO)));
    let error = foundation_deploy(ctx, plan).expect_err("the deploy needs a host");
    assert!(matches!(error, DeployError::HostNotConfigured { .. }));
}

/// The plan names the three Durable Object classes, the model slave binding and
/// the Model API binding.
#[test]
fn sandbox_worker_plan_shape() {
    let plan = DeploymentPlan::sandbox_worker();
    let classes: Vec<&str> = plan
        .bindings
        .iter()
        .filter_map(Binding::class_name)
        .collect();
    assert_eq!(classes, vec!["SandboxDo", "SandboxIndexDo", "ModelSlaveDo"]);
    assert!(plan.bindings.iter().any(|binding| matches!(
        binding,
        Binding::Ai { name } if name == "AI"
    )));
    assert_eq!(plan.containers.len(), 1);
    assert_eq!(plan.containers[0].class_name, "SandboxDo");
    assert_eq!(plan.secrets.len(), 1);
    assert_eq!(plan.secrets[0].name, "SANDBOX_TOKEN");
    assert_eq!(plan.secrets[0].source, SecretSource::WorkerSecret);
    assert_eq!(validate(&plan), Ok(()), "the sandbox plan is valid");
}
