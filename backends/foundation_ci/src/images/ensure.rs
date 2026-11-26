//! Reuse an image by digest or build it with the configured parent and export type.
use super::{prepare_plan, registry, report, Plan, Prepare};
use crate::CiEnvironment;
use clap::{Args, ValueEnum};
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult, TaskSet};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::Path,
};

#[derive(ValueEnum, Clone, Copy)]
enum LocalOutput {
    Oci,
    Docker,
}
#[derive(Args)]
struct Ensure {
    #[command(flatten)]
    prepare: Prepare,
    #[arg(long, default_value = "docker")]
    local_output: LocalOutput,
    #[arg(long, default_value = "")]
    parent_oci: String,
    #[arg(long)]
    push: bool,
    #[arg(long)]
    force: bool,
    #[arg(long)]
    no_cache: bool,
}
#[derive(Serialize)]
struct ResultRecord<'a> {
    #[serde(rename = "ref")]
    reference: String,
    #[serde(rename = "tag-ref")]
    tag_ref: &'a str,
    digest: &'a str,
    hash: &'a str,
    repository: &'a str,
    source: &'static str,
    built: bool,
    oci: String,
    tarball: String,
}
#[derive(Deserialize)]
struct Metadata {
    #[serde(rename = "containerimage.digest")]
    digest: String,
}

pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add(
        "ensure",
        "Reuse or build an image and return its digest",
        run,
    )?;
    set.expose("ensure", &["image-ensure"])
}
fn output_field(path: &Path) -> Result<String, TaskError> {
    let text = path
        .to_str()
        .ok_or_else(|| TaskError::operation("Invalid image output path"))?;
    // Buildx reads each exporter option as a CSV field.
    Ok(format!("\"dest={}\"", text.replace('"', "\"\"")))
}
async fn build(
    ctx: &TaskContext,
    host: &CiEnvironment,
    plan: &Plan,
    args: &Ensure,
    stage: &Path,
) -> Result<String, TaskError> {
    let mut cmd = ProcessCommand::new("docker")
        .args([
            "buildx",
            "build",
            "--pull",
            "--provenance=false",
            "--sbom=false",
            "--platform",
            plan.platform,
            "--tag",
            &plan.reference,
            "--file",
        ])
        .arg(&plan.dockerfile)
        .arg("--metadata-file")
        .arg(stage.join("metadata.json"))
        .arg("--cache-from")
        .arg(format!("type=registry,ref={}", plan.cache_ref));
    if args.push {
        cmd = cmd
            .args(["--output", "type=registry", "--cache-to"])
            .arg(format!(
                "type=registry,ref={},mode=max,image-manifest=true,oci-mediatypes=true",
                plan.cache_ref
            ));
    } else {
        let output = match args.local_output {
            LocalOutput::Oci => format!("type=oci,{},tar=false", output_field(&stage.join("oci"))?),
            LocalOutput::Docker => {
                format!("type=docker,{}", output_field(&stage.join("image.tar"))?)
            }
        };
        cmd = cmd.arg("--output").arg(output);
    }
    if !args.parent_oci.is_empty() {
        if args.prepare.parent.is_empty() {
            return Err(TaskError::operation("--parent-oci requires --parent"));
        }
        cmd = cmd.arg("--build-context").arg(format!(
            "{}=oci-layout://{}",
            args.prepare.parent, args.parent_oci
        ));
    }
    for arg in &plan.arguments {
        cmd = cmd.arg("--build-arg").arg(arg);
    }
    if args.no_cache {
        cmd = cmd.arg("--no-cache");
    }
    if host
        .get("PLATFORM_IMAGE_TOKEN")
        .is_some_and(|v| !v.is_empty())
    {
        cmd = cmd.args(["--secret", "id=github_token,env=PLATFORM_IMAGE_TOKEN"]);
    }
    ctx.exec(cmd.arg(&plan.context)).await?;
    let metadata: Metadata = serde_json::from_reader(BufReader::new(
        fs::File::open(stage.join("metadata.json")).map_err(TaskError::operation)?,
    ))
    .map_err(TaskError::operation)?;
    if metadata.digest.is_empty() {
        return Err(TaskError::operation("Image build returned no digest"));
    }
    Ok(metadata.digest)
}
fn summarize_inputs(path: &Path, root: &Path, reference: &str) -> TaskResult {
    let mut out = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(TaskError::operation)?;
    writeln!(
        out,
        "### Image inputs

Inputs hashed into `{reference}`

```"
    )
    .map_err(TaskError::operation)?;
    let inputs = fs::File::open(root.join("inputs.txt")).map_err(TaskError::operation)?;
    for line in BufReader::new(inputs).lines() {
        let line = line.map_err(TaskError::operation)?;
        if let Some((fields, _)) = line.rsplit_once('\t') {
            writeln!(out, "{fields}").map_err(TaskError::operation)?;
        }
    }
    writeln!(out, "```").map_err(TaskError::operation)
}
async fn run(ctx: TaskContext, mut args: Ensure) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let out = host.root.join(&args.prepare.out);
    fs::create_dir_all(&out).map_err(TaskError::operation)?;
    let out = out.canonicalize().map_err(TaskError::operation)?;
    let stage = tempfile::tempdir_in(&out).map_err(TaskError::operation)?;
    args.prepare.out = stage.path().to_owned();
    let mut plan = prepare_plan(&ctx, &args.prepare).await?;
    if args.prepare.github_output {
        if let Some(path) = host.get("GITHUB_STEP_SUMMARY").filter(|v| !v.is_empty()) {
            summarize_inputs(Path::new(&path), stage.path(), &plan.reference)?;
        }
    }
    let existing = if args.force {
        None
    } else {
        let seen = registry::lookup(&ctx, &plan.reference).await?;
        seen.exists.then_some(seen.digest)
    };
    let built = existing.is_none();
    let digest = if let Some(value) = existing {
        value
    } else {
        build(&ctx, &host, &plan, &args, stage.path()).await?
    };
    let local = built && !args.push;
    let mut pairs = vec![
        ("context", out.join("context")),
        ("plan.json", out.join("plan.json")),
        ("inputs.txt", out.join("inputs.txt")),
    ];
    let (mut oci, mut tarball) = (String::new(), String::new());
    if local {
        match args.local_output {
            LocalOutput::Oci => {
                let path = out.join("oci");
                oci = format!("{}@{digest}", path.display());
                pairs.push(("oci", path));
            }
            LocalOutput::Docker => {
                let path = out.join("image.tar");
                tarball = path.display().to_string();
                pairs.push(("image.tar", path));
            }
        }
    }
    plan.context = out.join("context");
    plan.dockerfile = plan.context.join("Dockerfile");
    fs::write(
        stage.path().join("plan.json"),
        serde_json::to_vec_pretty(&plan).map_err(TaskError::operation)?,
    )
    .map_err(TaskError::operation)?;
    crate::staging::publish(stage, &out, pairs, false)?;
    let source = if local { "local" } else { "registry" };
    let reference = if local {
        plan.reference.clone()
    } else {
        format!("{}@{digest}", plan.repository)
    };
    if args.prepare.github_output {
        if let Some(path) = host.get("GITHUB_STEP_SUMMARY") {
            writeln!(
                fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .map_err(TaskError::operation)?,
                "- `{reference}` ({source}, built: {built})"
            )
            .map_err(TaskError::operation)?;
        }
    }
    report(
        &host,
        &ResultRecord {
            reference,
            tag_ref: &plan.reference,
            digest: &digest,
            hash: &plan.hash,
            repository: &plan.repository,
            source,
            built,
            oci,
            tarball,
        },
        args.prepare.github_output,
    )
}
