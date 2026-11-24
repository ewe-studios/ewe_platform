//! Run project-owned Cloudflare operations with the installed Wrangler command.
use crate::CiEnvironment;
use clap::Args;
use foundation_tasks::{
    OutputStream, ProcessCommand, TaskContext, TaskEnv, TaskError, TaskResult, TaskSet,
};

#[derive(Args)]
struct Empty {}

#[derive(Args)]
struct Database {
    /// Name of the D1 database in the project configuration.
    #[arg(default_value = "foundation-db")]
    name: String,
}

#[derive(Args)]
struct Bucket {
    /// Name of the R2 bucket.
    #[arg(default_value = "foundation-bucket")]
    name: String,
}

fn pnpm(ctx: &TaskContext) -> Result<ProcessCommand, TaskError> {
    let host = ctx.require::<CiEnvironment>()?;
    Ok(ProcessCommand::new("pnpm").current_dir(&host.root))
}

fn wrangler(ctx: &TaskContext) -> Result<ProcessCommand, TaskError> {
    Ok(pnpm(ctx)?.args(["exec", "wrangler"]))
}

async fn environment(ctx: TaskContext, _: Empty) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let output = &ctx.require::<TaskEnv>()?.output;
    for key in [
        "DEPLOYMENT_D1_DATABASE_ID",
        "DEPLOYMENT_R2_BUCKET",
        "CLOUDFLARE_API_TOKEN",
        "CLOUDFLARE_ACCOUNT_ID",
        "D1_INTEGRATION_TEST",
        "R2_INTEGRATION_TEST",
    ] {
        let value = if key == "CLOUDFLARE_API_TOKEN" {
            host.get(key)
                .filter(|value| !value.is_empty())
                .map(|_| "set (hidden)".to_owned())
        } else {
            host.text(key)
        };
        let line = format!("{key}: {}\n", value.as_deref().unwrap_or("not set"));
        output
            .write(OutputStream::Stdout, line.as_bytes())
            .await
            .map_err(TaskError::operation)?;
    }
    Ok(())
}

/// Register Cloudflare tool installation, inventory, and resource commands.
///
/// # Errors
/// Returns a task registration error if a command name is invalid or duplicated.
pub fn set() -> Result<TaskSet, TaskError> {
    let mut tasks = TaskSet::new("foundation_ci_cloudflare").with_namespace("cf");
    tasks.add("env", "Show Cloudflare environment settings", environment)?;
    tasks.add(
        "install",
        "Install the project's Cloudflare tools",
        |ctx, _: Empty| async move { ctx.exec(pnpm(&ctx)?.arg("install")).await },
    )?;
    tasks.add(
        "whoami",
        "Show the active Cloudflare account",
        |ctx, _: Empty| async move { ctx.exec(wrangler(&ctx)?.arg("whoami")).await },
    )?;
    tasks.add(
        "list",
        "List D1 databases and R2 buckets",
        |ctx, _: Empty| async move {
            let tool = wrangler(&ctx)?;
            ctx.exec(tool.clone().args(["d1", "list"])).await?;
            ctx.exec(tool.args(["r2", "bucket", "list"])).await
        },
    )?;
    tasks.add(
        "d1:create",
        "Create a D1 database",
        |ctx, database: Database| async move {
            ctx.exec(wrangler(&ctx)?.args(["d1", "create"]).arg(database.name))
                .await
        },
    )?;
    tasks.add(
        "d1:migrate",
        "Apply pending local D1 migrations",
        |ctx, database: Database| async move {
            ctx.exec(
                wrangler(&ctx)?
                    .args(["d1", "migrations", "apply"])
                    .arg(database.name)
                    .arg("--local"),
            )
            .await
        },
    )?;
    tasks.add(
        "r2:create",
        "Create an R2 bucket",
        |ctx, bucket: Bucket| async move {
            ctx.exec(
                wrangler(&ctx)?
                    .args(["r2", "bucket", "create"])
                    .arg(bucket.name),
            )
            .await
        },
    )?;
    Ok(tasks)
}
