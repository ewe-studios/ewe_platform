//! Prove AWS KMS and GCP Cloud KMS against the real services (spec 92
//! feature 04).
//!
//! The task creates two AWS KMS keys (one right, one wrong) and two GCP Cloud
//! KMS keys in one key ring. It runs the four gated tests of `foundation_auth`
//! and then removes the keys again.
//!
//! The teardown always runs, also when a test fails. `--keep` is the only way
//! to skip it, for debugging. The AWS keys are scheduled for deletion with a
//! 7-day window. The GCP key versions are destroyed, and the key is deleted.
//!
//! A Google Cloud key ring cannot be deleted. The ring `ewe-live-test` stays
//! in the project. It costs nothing and holds no key material. Destroy the
//! whole scratch project to remove it.
//!
//! The task is opt-in: it fails when a value is missing instead of running
//! against an unintended account. Secrets stay in memory and in the child
//! process environment; the access token is never printed.

use crate::CiEnvironment;
use clap::Args;
use foundation_tasks::{
    OutputStream, ProcessCommand, TaskContext, TaskEnv, TaskError, TaskOutput, TaskResult, TaskSet,
};
use futures_util::future::BoxFuture;
use std::{
    io,
    sync::{Arc, Mutex},
};
use zeroize::Zeroizing;

const AWS_ALIAS: &str = "alias/ewe-live-test";
const AWS_WRONG_ALIAS: &str = "alias/ewe-live-test-wrong";
const RING: &str = "ewe-live-test";
const GCP_KEY: &str = "ewe-live-test";
const GCP_WRONG_KEY: &str = "ewe-live-test-wrong";
/// The filter of the four live tests. It matches `aws_live_*` and `gcp_live_*`.
const TEST_FILTER: &str = "live";
const TEST_COUNT: usize = 4;

#[derive(Args)]
struct LiveKms {
    /// AWS region. Defaults to `AWS_REGION`, then `AWS_DEFAULT_REGION`.
    #[arg(long)]
    region: Option<String>,
    /// GCP project. Defaults to `GCP_PROJECT`.
    #[arg(long)]
    project: Option<String>,
    /// GCP location. Defaults to `GCP_LOCATION`.
    #[arg(long)]
    location: Option<String>,
    /// Keep the keys for debugging; skip the teardown.
    #[arg(long)]
    keep: bool,
}

pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add(
        "live-kms",
        "Prove AWS and GCP KMS against the real services",
        run,
    )?;
    set.expose("live-kms", &["live-kms"])
}

/// The resources that the teardown must remove. A field is `Some` once the
/// resource exists.
#[derive(Default)]
struct Resources {
    key_id: Option<String>,
    wrong_key_id: Option<String>,
    key_arn: Option<String>,
    wrong_key_arn: Option<String>,
    gcp_key_name: Option<String>,
    gcp_wrong_key_name: Option<String>,
    passed: usize,
    failed: usize,
}

/// Echo a process to the task output, or hide its standard output.
#[derive(Clone, Copy)]
enum Mode {
    Echo,
    /// Keep standard output out of the log (an access token) but store it.
    Quiet,
}

/// A task output sink that stores the child output.
struct Tee {
    forward: Arc<dyn TaskOutput>,
    mode: Mode,
    stdout: Mutex<Vec<u8>>,
    stderr: Mutex<Vec<u8>>,
}

impl Tee {
    fn new(forward: Arc<dyn TaskOutput>, mode: Mode) -> Self {
        Self {
            forward,
            mode,
            stdout: Mutex::new(Vec::new()),
            stderr: Mutex::new(Vec::new()),
        }
    }

    fn take(&self) -> (String, String) {
        let text = |bytes: &Mutex<Vec<u8>>| {
            bytes
                .lock()
                .map_or_default(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        };
        (text(&self.stdout), text(&self.stderr))
    }
}

impl TaskOutput for Tee {
    fn write<'a>(&'a self, stream: OutputStream, bytes: &'a [u8]) -> BoxFuture<'a, io::Result<()>> {
        Box::pin(async move {
            match stream {
                OutputStream::Stdout => {
                    self.stdout
                        .lock()
                        .map_err(|_| io::Error::other("output lock poisoned"))?
                        .extend_from_slice(bytes);
                    if matches!(self.mode, Mode::Quiet) {
                        return Ok(());
                    }
                }
                OutputStream::Stderr => {
                    self.stderr
                        .lock()
                        .map_err(|_| io::Error::other("output lock poisoned"))?
                        .extend_from_slice(bytes);
                }
            }
            self.forward.write(stream, bytes).await
        })
    }

    fn flush(&self) -> BoxFuture<'_, io::Result<()>> {
        Box::pin(async move { self.forward.flush().await })
    }
}

/// The result of one child process.
struct Captured {
    ok: bool,
    stdout: String,
    stderr: String,
}

async fn capture(
    ctx: &TaskContext,
    command: ProcessCommand,
    mode: Mode,
) -> Result<Captured, TaskError> {
    let output = ctx.require::<TaskEnv>()?.output.clone();
    let tee = Arc::new(Tee::new(output, mode));
    let result = ctx.exec_with_output(command, tee.clone()).await;
    let (stdout, stderr) = tee.take();
    match result {
        Ok(()) => Ok(Captured {
            ok: true,
            stdout,
            stderr,
        }),
        Err(TaskError::Exit { .. }) => Ok(Captured {
            ok: false,
            stdout,
            stderr,
        }),
        Err(error) => Err(error),
    }
}

/// Run a command and return its trimmed standard output, or an error that
/// names the step and its standard error.
async fn capture_text(
    ctx: &TaskContext,
    command: ProcessCommand,
    step: &str,
) -> Result<String, TaskError> {
    let captured = capture(ctx, command, Mode::Echo).await?;
    if !captured.ok {
        return Err(TaskError::operation(format!(
            "{step} failed: {}",
            captured.stderr.trim()
        )));
    }
    Ok(captured.stdout.trim().to_string())
}

fn aws(region: &str, args: &[&str]) -> ProcessCommand {
    ProcessCommand::new("aws")
        .arg("--region")
        .arg(region)
        .args(args.iter().copied())
}

fn gcloud(project: &str, location: &str, args: &[&str]) -> ProcessCommand {
    ProcessCommand::new("gcloud")
        .args(args.iter().copied())
        .arg("--project")
        .arg(project)
        .arg("--location")
        .arg(location)
}

fn already_exists(captured: &Captured) -> bool {
    let text = format!("{}\n{}", captured.stdout, captured.stderr);
    text.contains("ALREADY_EXISTS") || text.to_lowercase().contains("already exists")
}

async fn create_aws_key(
    ctx: &TaskContext,
    region: &str,
    description: &str,
) -> Result<String, TaskError> {
    let command = aws(
        region,
        &[
            "kms",
            "create-key",
            "--description",
            description,
            "--tags",
            "TagKey=purpose,TagValue=ewe-live-test",
            "TagKey=created-by,TagValue=foundation-ci",
            "--query",
            "KeyMetadata.KeyId",
            "--output",
            "text",
        ],
    );
    capture_text(ctx, command, "aws kms create-key").await
}

async fn ensure_alias(ctx: &TaskContext, region: &str, alias: &str, key_id: &str) -> TaskResult {
    println!("live-kms: setting {alias}");
    let created = capture(
        ctx,
        aws(
            region,
            &[
                "kms",
                "create-alias",
                "--alias-name",
                alias,
                "--target-key-id",
                key_id,
            ],
        ),
        Mode::Echo,
    )
    .await?;
    if created.ok {
        return Ok(());
    }
    // An interrupted run leaves the alias behind. Point it at the new key.
    let updated = capture(
        ctx,
        aws(
            region,
            &[
                "kms",
                "update-alias",
                "--alias-name",
                alias,
                "--target-key-id",
                key_id,
            ],
        ),
        Mode::Echo,
    )
    .await?;
    if updated.ok {
        Ok(())
    } else {
        Err(TaskError::operation(format!(
            "alias {alias}: {}",
            updated.stderr.trim()
        )))
    }
}

async fn ensure_keyring(ctx: &TaskContext, project: &str, location: &str) -> TaskResult {
    println!("live-kms: creating the GCP key ring {RING} if it is absent");
    let created = capture(
        ctx,
        gcloud(project, location, &["kms", "keyrings", "create", RING]),
        Mode::Echo,
    )
    .await?;
    if created.ok || already_exists(&created) {
        Ok(())
    } else {
        Err(TaskError::operation(format!(
            "gcloud key ring {RING}: {}",
            created.stderr.trim()
        )))
    }
}

async fn ensure_gcp_key(ctx: &TaskContext, project: &str, location: &str, key: &str) -> TaskResult {
    println!("live-kms: creating the GCP key {key} if it is absent");
    let created = capture(
        ctx,
        gcloud(
            project,
            location,
            &[
                "kms",
                "keys",
                "create",
                key,
                "--purpose=encryption",
                "--keyring",
                RING,
                "--labels=purpose=ewe-live-test",
            ],
        ),
        Mode::Echo,
    )
    .await?;
    if created.ok || already_exists(&created) {
        Ok(())
    } else {
        Err(TaskError::operation(format!(
            "gcloud key {key}: {}",
            created.stderr.trim()
        )))
    }
}

/// Make sure the key has one enabled version (#1113).
///
/// WHY: Google Cloud keeps the key entry after a version is destroyed, so a
/// second run finds the key, tolerates `ALREADY_EXISTS`, and then cannot
/// encrypt: every version is `DESTROY_SCHEDULED`. A version takes a day to
/// finish destruction, so the task is not repeatable without this step.
///
/// WHAT/HOW: list the versions; when none is `ENABLED`, create one. The first
/// run creates version 1 with the key and this returns at once.
async fn ensure_gcp_version(
    ctx: &TaskContext,
    project: &str,
    location: &str,
    key: &str,
) -> TaskResult {
    let listed = capture(
        ctx,
        gcloud(
            project,
            location,
            &[
                "kms",
                "keys",
                "versions",
                "list",
                "--key",
                key,
                "--keyring",
                RING,
                "--format=value(state)",
            ],
        ),
        Mode::Echo,
    )
    .await?;
    if !listed.ok {
        return Err(TaskError::operation(format!(
            "gcloud key {key} versions: {}",
            listed.stderr.trim()
        )));
    }
    if listed.stdout.lines().any(|line| line.trim() == "ENABLED") {
        return Ok(());
    }
    println!("live-kms: creating a GCP key version for {key}: no version is enabled");
    let created = capture(
        ctx,
        gcloud(
            project,
            location,
            &[
                "kms",
                "keys",
                "versions",
                "create",
                "--key",
                key,
                "--keyring",
                RING,
            ],
        ),
        Mode::Echo,
    )
    .await?;
    if created.ok {
        Ok(())
    } else {
        Err(TaskError::operation(format!(
            "gcloud key version {key}: {}",
            created.stderr.trim()
        )))
    }
}

/// The access token. `GCP_ACCESS_TOKEN` wins, then `gcloud auth
/// print-access-token`. The token never enters the log.
async fn access_token(
    ctx: &TaskContext,
    host: &CiEnvironment,
) -> Result<Zeroizing<String>, TaskError> {
    if let Some(token) = host.text("GCP_ACCESS_TOKEN") {
        return Ok(Zeroizing::new(token));
    }
    let captured = capture(
        ctx,
        ProcessCommand::new("gcloud").args(["auth", "print-access-token"]),
        Mode::Quiet,
    )
    .await?;
    if !captured.ok {
        return Err(TaskError::operation(format!(
            "gcloud auth print-access-token failed: {}",
            captured.stderr.trim()
        )));
    }
    let token = Zeroizing::new(captured.stdout.trim().to_string());
    if token.is_empty() {
        return Err(TaskError::operation(
            "gcloud returned an empty access token",
        ));
    }
    Ok(token)
}

/// The AWS credentials for the test process.
///
/// The AWS KMS backend of `foundation_auth` reads `AWS_ACCESS_KEY_ID` and
/// `AWS_SECRET_ACCESS_KEY` from the environment, then the instance metadata
/// service. It does not read the AWS CLI login cache. The task therefore takes
/// the credentials the CLI already resolved and passes them to the child
/// process only. The values are never printed, and they are zeroed on drop.
struct AwsCredentials {
    access_key: Zeroizing<String>,
    secret_key: Zeroizing<String>,
    session_token: Option<Zeroizing<String>>,
}

async fn aws_credentials(
    ctx: &TaskContext,
    host: &CiEnvironment,
) -> Result<AwsCredentials, TaskError> {
    if let (Some(access_key), Some(secret_key)) = (
        host.text("AWS_ACCESS_KEY_ID"),
        host.text("AWS_SECRET_ACCESS_KEY"),
    ) {
        return Ok(AwsCredentials {
            access_key: Zeroizing::new(access_key),
            secret_key: Zeroizing::new(secret_key),
            session_token: host.text("AWS_SESSION_TOKEN").map(Zeroizing::new),
        });
    }
    let captured = capture(
        ctx,
        ProcessCommand::new("aws").args(["configure", "export-credentials", "--format", "env"]),
        Mode::Quiet,
    )
    .await?;
    if !captured.ok {
        return Err(TaskError::operation(format!(
            "aws configure export-credentials failed: {}",
            captured.stderr.trim()
        )));
    }
    let (mut access_key, mut secret_key, mut session_token) = (None, None, None);
    for line in captured.stdout.lines() {
        let Some(rest) = line.trim().strip_prefix("export ") else {
            continue;
        };
        let Some((name, value)) = rest.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches(['"', '\'']);
        match name {
            "AWS_ACCESS_KEY_ID" => access_key = Some(Zeroizing::new(value.to_string())),
            "AWS_SECRET_ACCESS_KEY" => secret_key = Some(Zeroizing::new(value.to_string())),
            "AWS_SESSION_TOKEN" => session_token = Some(Zeroizing::new(value.to_string())),
            _ => {}
        }
    }
    let (Some(access_key), Some(secret_key)) = (access_key, secret_key) else {
        return Err(TaskError::operation(
            "aws configure export-credentials returned no access key pair",
        ));
    };
    Ok(AwsCredentials {
        access_key,
        secret_key,
        session_token,
    })
}

fn test_command(
    host: &CiEnvironment,
    region: &str,
    project: &str,
    location: &str,
    resources: &Resources,
    credentials: &AwsCredentials,
    token: &str,
) -> Result<ProcessCommand, TaskError> {
    let field = |value: &Option<String>, name: &str| {
        value
            .clone()
            .ok_or_else(|| TaskError::operation(format!("the live KMS run has no {name}")))
    };
    let mut command = ProcessCommand::new("cargo")
        .current_dir(&host.root)
        .args([
            "test",
            "-p",
            "foundation_auth",
            "--features",
            "testing",
            "--profile",
            "uat",
            "--test",
            "kms_tests",
            TEST_FILTER,
            "--",
            "--nocapture",
        ])
        .env("AWS_REGION", region)
        .env("AWS_DEFAULT_REGION", region)
        .env("AWS_ACCESS_KEY_ID", credentials.access_key.as_str())
        .env("AWS_SECRET_ACCESS_KEY", credentials.secret_key.as_str())
        .env("AWS_KMS_KEY_ARN", field(&resources.key_arn, "AWS key ARN")?)
        .env(
            "AWS_WRONG_KEY_ARN",
            field(&resources.wrong_key_arn, "wrong AWS key ARN")?,
        )
        .env(
            "GCP_KMS_KEY_NAME",
            field(&resources.gcp_key_name, "GCP key name")?,
        )
        .env(
            "GCP_WRONG_KEY_NAME",
            field(&resources.gcp_wrong_key_name, "wrong GCP key name")?,
        )
        .env("GCP_KMS_PROJECT", project)
        .env("GCP_KMS_LOCATION", location)
        .env("GCP_ACCESS_TOKEN", token);
    if let Some(session_token) = &credentials.session_token {
        command = command.env("AWS_SESSION_TOKEN", session_token.as_str());
    }
    Ok(command)
}

/// Record each `test <name> ... ok|FAILED` line of the test run.
fn record_test_lines(captured: &Captured, resources: &mut Resources) {
    for line in captured.stdout.lines().chain(captured.stderr.lines()) {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("test ") else {
            continue;
        };
        let Some((name, result)) = rest.split_once(" ... ") else {
            continue;
        };
        match result {
            "ok" => {
                resources.passed += 1;
                println!("live-kms: PASS {name}");
            }
            "FAILED" => {
                resources.failed += 1;
                println!("live-kms: FAIL {name}");
            }
            _ => {}
        }
    }
}

async fn prove(
    ctx: &TaskContext,
    host: &CiEnvironment,
    region: &str,
    project: &str,
    location: &str,
    resources: &mut Resources,
) -> TaskResult {
    println!("live-kms: creating the AWS KMS keys in {region}");
    resources.key_id = Some(create_aws_key(ctx, region, "ewe live test key").await?);
    resources.wrong_key_id = Some(create_aws_key(ctx, region, "ewe live test wrong key").await?);
    let key_id = resources.key_id.clone().unwrap_or_default();
    let wrong_key_id = resources.wrong_key_id.clone().unwrap_or_default();
    resources.key_arn = Some(
        capture_text(
            ctx,
            aws(
                region,
                &[
                    "kms",
                    "describe-key",
                    "--key-id",
                    &key_id,
                    "--query",
                    "KeyMetadata.Arn",
                    "--output",
                    "text",
                ],
            ),
            "aws kms describe-key",
        )
        .await?,
    );
    resources.wrong_key_arn = Some(
        capture_text(
            ctx,
            aws(
                region,
                &[
                    "kms",
                    "describe-key",
                    "--key-id",
                    &wrong_key_id,
                    "--query",
                    "KeyMetadata.Arn",
                    "--output",
                    "text",
                ],
            ),
            "aws kms describe-key",
        )
        .await?,
    );
    ensure_alias(ctx, region, AWS_ALIAS, &key_id).await?;
    ensure_alias(ctx, region, AWS_WRONG_ALIAS, &wrong_key_id).await?;
    println!(
        "live-kms: AWS key {}, wrong key {}",
        resources.key_arn.as_deref().unwrap_or("<none>"),
        resources.wrong_key_arn.as_deref().unwrap_or("<none>")
    );

    println!("live-kms: creating the GCP Cloud KMS keys in {project}/{location}");
    ensure_keyring(ctx, project, location).await?;
    ensure_gcp_key(ctx, project, location, GCP_KEY).await?;
    ensure_gcp_key(ctx, project, location, GCP_WRONG_KEY).await?;
    ensure_gcp_version(ctx, project, location, GCP_KEY).await?;
    ensure_gcp_version(ctx, project, location, GCP_WRONG_KEY).await?;
    let full_name = |key: &str| {
        format!("projects/{project}/locations/{location}/keyRings/{RING}/cryptoKeys/{key}")
    };
    resources.gcp_key_name = Some(full_name(GCP_KEY));
    resources.gcp_wrong_key_name = Some(full_name(GCP_WRONG_KEY));
    println!(
        "live-kms: GCP key {}, wrong key {}",
        resources.gcp_key_name.as_deref().unwrap_or("<none>"),
        resources.gcp_wrong_key_name.as_deref().unwrap_or("<none>")
    );

    let credentials = aws_credentials(ctx, host).await?;
    let token = access_token(ctx, host).await?;
    println!("live-kms: running the {TEST_COUNT} gated tests");
    let command = test_command(
        host,
        region,
        project,
        location,
        resources,
        &credentials,
        token.as_str(),
    )?;
    let captured = capture(ctx, command, Mode::Echo).await?;
    record_test_lines(&captured, resources);

    let skipped = captured.stdout.contains("skip: set") || captured.stderr.contains("skip: set");
    if skipped {
        return Err(TaskError::operation(
            "a live KMS test skipped even though the task set its variables; a skip is not a pass",
        ));
    }
    if !captured.ok || resources.failed > 0 || resources.passed != TEST_COUNT {
        return Err(TaskError::operation(format!(
            "the live KMS tests failed: {} passed, {} failed, exit {}",
            resources.passed,
            resources.failed,
            if captured.ok { "ok" } else { "error" }
        )));
    }
    println!("live-kms: all {TEST_COUNT} tests passed");
    Ok(())
}

impl Resources {
    /// Remove every resource that exists. It records a failure for each AWS key
    /// that cannot be scheduled for deletion and each GCP version that cannot
    /// be destroyed, and keeps going, so one failure does not hide another.
    async fn teardown(
        &self,
        ctx: &TaskContext,
        region: &str,
        project: &str,
        location: &str,
    ) -> TaskResult {
        println!("live-kms: teardown");
        let mut failures = Vec::new();
        if let Err(error) = self.teardown_aws(ctx, region, &mut failures).await {
            failures.push(format!("aws teardown: {error}"));
        }
        if let Err(error) = self
            .teardown_gcp(ctx, project, location, &mut failures)
            .await
        {
            failures.push(format!("gcp teardown: {error}"));
        }
        println!(
            "live-kms: key ring {RING} stays in {project}: Google Cloud cannot delete a key ring"
        );
        if failures.is_empty() {
            Ok(())
        } else {
            Err(TaskError::operation(format!(
                "teardown failed: {}",
                failures.join("; ")
            )))
        }
    }

    async fn teardown_aws(
        &self,
        ctx: &TaskContext,
        region: &str,
        failures: &mut Vec<String>,
    ) -> TaskResult {
        for (label, key_id) in [
            ("aws key", self.key_id.as_deref()),
            ("aws wrong key", self.wrong_key_id.as_deref()),
        ] {
            let Some(key_id) = key_id else {
                continue;
            };
            let scheduled = capture(
                ctx,
                aws(
                    region,
                    &[
                        "kms",
                        "schedule-key-deletion",
                        "--key-id",
                        key_id,
                        "--pending-window-in-days",
                        "7",
                    ],
                ),
                Mode::Echo,
            )
            .await?;
            if scheduled.ok {
                println!("live-kms: {label} {key_id}: PendingDeletion (7 days)");
            } else {
                failures.push(format!("{label} {key_id}: {}", scheduled.stderr.trim()));
            }
        }
        for alias in [AWS_ALIAS, AWS_WRONG_ALIAS] {
            let deleted = capture(
                ctx,
                aws(region, &["kms", "delete-alias", "--alias-name", alias]),
                Mode::Echo,
            )
            .await?;
            if deleted.ok {
                println!("live-kms: {alias}: deleted");
            } else if !deleted.stderr.contains("NotFound") {
                println!(
                    "live-kms: {alias}: delete failed: {}",
                    deleted.stderr.trim()
                );
            }
        }
        Ok(())
    }

    async fn teardown_gcp(
        &self,
        ctx: &TaskContext,
        project: &str,
        location: &str,
        failures: &mut Vec<String>,
    ) -> TaskResult {
        for (label, key, name) in [
            ("gcp key", GCP_KEY, self.gcp_key_name.as_deref()),
            (
                "gcp wrong key",
                GCP_WRONG_KEY,
                self.gcp_wrong_key_name.as_deref(),
            ),
        ] {
            if name.is_none() {
                continue;
            }
            let listed = capture(
                ctx,
                gcloud(
                    project,
                    location,
                    &[
                        "kms",
                        "keys",
                        "versions",
                        "list",
                        "--key",
                        key,
                        "--keyring",
                        RING,
                        "--format=value(name,state)",
                    ],
                ),
                Mode::Echo,
            )
            .await?;
            if !listed.ok {
                failures.push(format!("{label} {key} versions: {}", listed.stderr.trim()));
                continue;
            }
            for line in listed.stdout.lines().filter(|line| !line.trim().is_empty()) {
                // A version of an earlier run is already scheduled for
                // destruction; destroying it again is not an error (#1113).
                let mut fields = line.split('\t');
                let name = fields.next().unwrap_or_default().trim();
                let state = fields.next().unwrap_or_default().trim();
                if matches!(state, "DESTROY_SCHEDULED" | "DESTROYED") {
                    println!("live-kms: {label} {key} version {name}: already {state}");
                    continue;
                }
                let version_id = name.rsplit('/').next().unwrap_or(name).trim();
                let destroyed = capture(
                    ctx,
                    gcloud(
                        project,
                        location,
                        &[
                            "kms",
                            "keys",
                            "versions",
                            "destroy",
                            version_id,
                            "--key",
                            key,
                            "--keyring",
                            RING,
                            "--quiet",
                        ],
                    ),
                    Mode::Echo,
                )
                .await?;
                if destroyed.ok {
                    println!("live-kms: {label} {key} version {version_id}: DESTROY_SCHEDULED");
                } else {
                    failures.push(format!(
                        "{label} {key} version {version_id}: {}",
                        destroyed.stderr.trim()
                    ));
                }
            }
            let deleted = capture(
                ctx,
                gcloud(
                    project,
                    location,
                    &["kms", "keys", "delete", key, "--keyring", RING, "--quiet"],
                ),
                Mode::Echo,
            )
            .await?;
            if deleted.ok {
                println!("live-kms: {label} {key}: deleted");
            } else {
                // Cloud KMS keeps the key entry after its versions are
                // destroyed. The versions hold the key material.
                println!(
                    "live-kms: {label} {key}: key entry remains, versions are destroyed ({})",
                    deleted.stderr.trim()
                );
            }
        }
        Ok(())
    }
}

fn print_summary(
    resources: &Resources,
    region: &str,
    project: &str,
    location: &str,
    keep: bool,
    teardown_ok: bool,
) {
    println!("\nlive-kms summary");
    println!("  AWS region:  {region}");
    println!(
        "  AWS key:     {} (id {})",
        resources.key_arn.as_deref().unwrap_or("<none>"),
        resources.key_id.as_deref().unwrap_or("<none>")
    );
    println!(
        "  AWS wrong:   {} (id {})",
        resources.wrong_key_arn.as_deref().unwrap_or("<none>"),
        resources.wrong_key_id.as_deref().unwrap_or("<none>")
    );
    println!("  GCP project: {project}");
    println!("  GCP location:{location}");
    println!(
        "  GCP key:     {}",
        resources.gcp_key_name.as_deref().unwrap_or("<none>")
    );
    println!(
        "  GCP wrong:   {}",
        resources.gcp_wrong_key_name.as_deref().unwrap_or("<none>")
    );
    println!(
        "  tests:       {} passed, {} failed",
        resources.passed, resources.failed
    );
    let state = if keep {
        "skipped (--keep)".to_string()
    } else if teardown_ok {
        "done (see the lines above)".to_string()
    } else {
        "FAILED (see the lines above)".to_string()
    };
    println!("  teardown:    {state}");
}

fn resolve(
    flag: Option<String>,
    host: &CiEnvironment,
    variables: &[&str],
    flag_name: &str,
) -> Result<String, TaskError> {
    if let Some(value) = flag.filter(|value| !value.trim().is_empty()) {
        return Ok(value);
    }
    for variable in variables {
        if let Some(value) = host.text(variable) {
            return Ok(value);
        }
    }
    Err(TaskError::operation(format!(
        "live-kms needs {flag_name} or {}; the task is opt-in and does not pick an account",
        variables.join(" or ")
    )))
}

async fn run(ctx: TaskContext, args: LiveKms) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let region = resolve(
        args.region.clone(),
        &host,
        &["AWS_REGION", "AWS_DEFAULT_REGION"],
        "--region",
    )?;
    let project = resolve(args.project.clone(), &host, &["GCP_PROJECT"], "--project")?;
    let location = resolve(
        args.location.clone(),
        &host,
        &["GCP_LOCATION"],
        "--location",
    )?;
    println!("live-kms: AWS {region}, GCP {project}/{location}");

    let mut resources = Resources::default();
    let outcome = prove(&ctx, &host, &region, &project, &location, &mut resources).await;
    // This runs on every path, after a pass and after a failure. `--keep` is
    // the only way to skip it.
    let teardown = if args.keep {
        println!("live-kms: --keep set, the keys and aliases stay");
        Ok(())
    } else {
        resources.teardown(&ctx, &region, &project, &location).await
    };
    print_summary(
        &resources,
        &region,
        &project,
        &location,
        args.keep,
        teardown.is_ok(),
    );
    match (outcome, teardown) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => Err(TaskError::Cleanup {
            source: Box::new(error),
            cleanup: Box::new(cleanup),
        }),
    }
}
