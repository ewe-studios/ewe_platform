//! The product deploy-plan test of spec 77 feature 17, section 7.
//!
//! The test uses fakes: no network call and no container engine run. It proves
//! that both deploy adapters accept the product plan and that the plan names
//! `./Dockerfile.product`.

use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use foundation_auth::server_credential::Secret;
use foundation_context::Ctx;
use foundation_core::valtron::{block_on_future, SimulatedClock};
use foundation_deployment_cloudflare::deploy::DeployFuture;
use foundation_deployment_cloudflare::{
    validate, Binding, CloudflareDeployHost, CloudflareDeployer, CommandOutput, CommandRunner,
    CommandSpec, ContainerDeployment, ContainerImage, DeployError, DeploymentApi, DeploymentId,
    DeploymentPlan, DeploymentStatus, DoMigration, FoundationCloudflareDeployer, ImageBuilder,
    Promotion, Route, SecretSource, SecretStore, WorkerDeployment, WranglerCloudflareDeployer,
};

/// A unique temporary directory for the test.
fn temporary_directory(name: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "ewe-deploy-product-{}-{}-{name}",
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

/// A fake Cloudflare API that succeeds and records the Worker name.
#[derive(Clone, Default)]
struct FakeApi {
    promoted: Arc<Mutex<Option<String>>>,
}

impl DeploymentApi for FakeApi {
    fn ensure_container(
        &self,
        _worker_name: &str,
        _container: &ContainerDeployment,
        _tag: &str,
    ) -> DeployFuture<'_, Result<(), DeployError>> {
        Box::pin(async { Ok(()) })
    }

    fn upload_version(
        &self,
        _worker: &WorkerDeployment,
        _bindings: &[Binding],
        _migrations: &[DoMigration],
    ) -> DeployFuture<'_, Result<String, DeployError>> {
        Box::pin(async { Ok("version-1".to_string()) })
    }

    fn put_secret(
        &self,
        _worker_name: &str,
        _version: &str,
        _name: &str,
        _value: &Secret,
    ) -> DeployFuture<'_, Result<(), DeployError>> {
        Box::pin(async { Ok(()) })
    }

    fn set_route(
        &self,
        _worker_name: &str,
        _route: &Route,
    ) -> DeployFuture<'_, Result<(), DeployError>> {
        Box::pin(async { Ok(()) })
    }

    fn promote(
        &self,
        worker_name: &str,
        _version: &str,
    ) -> DeployFuture<'_, Result<Promotion, DeployError>> {
        *self.promoted.lock().expect("the promoted cell") = Some(worker_name.to_string());
        Box::pin(async {
            Ok(Promotion {
                worker_url: "https://ewe-sandbox.example.workers.dev".to_string(),
                deployment_id: DeploymentId::new("deploy-1"),
            })
        })
    }

    fn status(
        &self,
        _id: &DeploymentId,
    ) -> DeployFuture<'_, Result<DeploymentStatus, DeployError>> {
        Box::pin(async { Ok(DeploymentStatus::Live) })
    }

    fn rollback(&self, _id: &DeploymentId) -> DeployFuture<'_, Result<(), DeployError>> {
        Box::pin(async { Ok(()) })
    }
}

/// A fake image builder that records the Dockerfile it received.
#[derive(Clone, Default)]
struct FakeImages {
    built: Arc<Mutex<Vec<PathBuf>>>,
}

impl ImageBuilder for FakeImages {
    fn build(&self, dockerfile: &Path, _tag: &str) -> DeployFuture<'_, Result<(), DeployError>> {
        self.built
            .lock()
            .expect("the build log")
            .push(dockerfile.to_path_buf());
        Box::pin(async { Ok(()) })
    }

    fn push(&self, _tag: &str) -> DeployFuture<'_, Result<(), DeployError>> {
        Box::pin(async { Ok(()) })
    }
}

/// A fake command runner.
#[derive(Clone, Default)]
struct FakeCommands {
    deploy_directory: Arc<Mutex<Option<PathBuf>>>,
}

impl CommandRunner for FakeCommands {
    fn run(&self, spec: CommandSpec) -> DeployFuture<'_, Result<CommandOutput, DeployError>> {
        if spec.args == ["wrangler", "deploy"] {
            *self.deploy_directory.lock().expect("the deploy directory") = spec.cwd.clone();
            return Box::pin(async {
                Ok(CommandOutput {
                    code: 0,
                    stdout: "Deployed ewe-sandbox to https://ewe-sandbox.example.workers.dev"
                        .to_string(),
                    stderr: String::new(),
                })
            });
        }
        Box::pin(async {
            Ok(CommandOutput {
                code: 0,
                stdout: String::new(),
                stderr: String::new(),
            })
        })
    }
}

/// A fake secret store.
#[derive(Clone, Default)]
struct FakeSecrets;

impl SecretStore for FakeSecrets {
    fn get(
        &self,
        _name: &str,
        _source: &SecretSource,
    ) -> DeployFuture<'_, Result<Secret, DeployError>> {
        Box::pin(async { Ok(Secret::new("test-token")) })
    }
}

/// A host with every fake.
#[derive(Clone, Default)]
struct Fakes {
    api: FakeApi,
    images: FakeImages,
    commands: FakeCommands,
    secrets: FakeSecrets,
}

impl Fakes {
    fn host(&self) -> CloudflareDeployHost {
        CloudflareDeployHost::new()
            .with_api(Arc::new(self.api.clone()))
            .with_images(Arc::new(self.images.clone()))
            .with_commands(Arc::new(self.commands.clone()))
            .with_secrets(Arc::new(self.secrets.clone()))
    }
}

/// The product plan is the sandbox plan with one image change, and both
/// adapters accept it.
#[test]
fn product_deploy_plan() {
    let directory = temporary_directory("plan");
    fs::create_dir_all(directory.join("build")).expect("the build directory");
    fs::write(directory.join("build/index.js"), b"export default {}").expect("the main module");
    fs::write(directory.join("Dockerfile.product"), b"FROM ubuntu:24.04\n")
        .expect("the product Dockerfile");

    let mut plan = DeploymentPlan::sandbox_worker_product();
    plan.worker.main = directory.join("build/index.js");

    // The one difference from the sandbox plan.
    let plain = DeploymentPlan::sandbox_worker();
    assert_eq!(plan.worker.name, plain.worker.name);
    assert_eq!(
        plan.worker.compatibility_date,
        plain.worker.compatibility_date
    );
    assert_eq!(plan.bindings, plain.bindings);
    assert_eq!(plan.migrations, plain.migrations);
    assert_eq!(plan.routes, plain.routes);
    assert_eq!(plan.secrets, plain.secrets);
    assert_eq!(
        plan.containers[0].class_name,
        plain.containers[0].class_name
    );
    assert_eq!(plan.containers[0].name, plain.containers[0].name);
    assert_eq!(
        plan.containers[0].max_instances,
        plain.containers[0].max_instances
    );
    assert_eq!(
        plan.containers[0].instance_type,
        plain.containers[0].instance_type
    );
    assert_eq!(
        plan.containers[0].image,
        ContainerImage::Dockerfile(PathBuf::from("./Dockerfile.product"))
    );
    validate(&plan).expect("the product plan is valid");

    // The foundation adapter accepts the plan and builds its Dockerfile.
    let foundation = Fakes::default();
    let outcome = run({
        let ctx = context(foundation.host());
        let plan = plan.clone();
        async move { FoundationCloudflareDeployer::new().deploy(ctx, plan).await }
    })
    .expect("the foundation deploy");
    assert_eq!(
        outcome.worker_url,
        "https://ewe-sandbox.example.workers.dev"
    );
    assert_eq!(
        *foundation.api.promoted.lock().expect("the promoted cell"),
        Some("ewe-sandbox".to_string())
    );
    assert_eq!(
        *foundation.images.built.lock().expect("the build log"),
        vec![PathBuf::from("./Dockerfile.product")]
    );

    // The wrangler adapter accepts the same plan and writes the image path.
    let wrangler = Fakes::default();
    let wrangler_outcome = run({
        let ctx = context(wrangler.host());
        let plan = plan.clone();
        async move { WranglerCloudflareDeployer::new().deploy(ctx, plan).await }
    })
    .expect("the wrangler deploy");
    assert_eq!(wrangler_outcome.worker_url, outcome.worker_url);
    let deploy_directory = wrangler
        .commands
        .deploy_directory
        .lock()
        .expect("the deploy directory")
        .clone()
        .expect("the wrangler deploy directory");
    let toml = fs::read_to_string(deploy_directory.join("wrangler.toml"))
        .expect("the written wrangler.toml");
    assert!(
        toml.contains("image = \"./Dockerfile.product\""),
        "the wrangler.toml names the product image: {toml}"
    );
    assert!(
        toml.contains("name = \"ewe-sandbox\""),
        "the wrangler.toml names the same Worker: {toml}"
    );
}
