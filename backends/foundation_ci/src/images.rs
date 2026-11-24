//! Stage content-addressed container and virtual machine build contexts.
mod config;
mod ensure;
mod files;
mod inputs;
mod registry;
mod smoke;

use crate::CiEnvironment;
use clap::{Args, ValueEnum};
use config::Config;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult, TaskSet};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Layer {
    Base,
    Image,
    VmBase,
    VmImage,
}

#[derive(Args)]
struct MacOsImage {
    /// Apple release name accepted by Quickget.
    #[arg(default_value = "sonoma")]
    version: String,
}

#[derive(Args)]
struct Prepare {
    #[arg(long)]
    config: PathBuf,
    #[arg(long)]
    layer: Layer,
    #[arg(long, default_value = "default")]
    flavor: String,
    #[arg(long, default_value = "")]
    os: String,
    #[arg(long, default_value = "")]
    parent: String,
    #[arg(long)]
    out: PathBuf,
    #[arg(long, default_value = ".")]
    repo: PathBuf,
    #[arg(long)]
    unit: Option<PathBuf>,
    #[arg(long)]
    github_output: bool,
}

#[derive(Serialize)]
struct Plan {
    layer: Layer,
    flavor: String,
    hash: String,
    repository: String,
    tag: String,
    #[serde(rename = "ref")]
    reference: String,
    cache_ref: String,
    context: PathBuf,
    dockerfile: PathBuf,
    platform: &'static str,
    build_args: String,
    #[serde(skip)]
    arguments: Vec<String>,
    registry_host: String,
    registry_username: String,
}

/// Register the image context planner.
///
/// # Errors
/// Returns an error for a duplicate task name.
pub fn set() -> Result<TaskSet, TaskError> {
    let mut set = TaskSet::new("foundation_images").with_namespace("images");
    set.add(
        "download-macos",
        "Download the selected macOS virtual machine image",
        |ctx, args: MacOsImage| async move {
            let host = ctx.require::<CiEnvironment>()?;
            ctx.exec(
                ProcessCommand::new("quickget")
                    .current_dir(&host.root)
                    .arg("macos")
                    .arg(args.version),
            )
            .await
        },
    )?;
    set.add("prepare", "Prepare an image build context", prepare)?;
    set.expose("prepare", &["image-prepare"])?;
    set.add(
        "settings",
        "Read the image registry settings",
        |ctx, args| std::future::ready(settings(&ctx, args)),
    )?;
    set.expose("settings", &["image-settings"])?;
    smoke::register(&mut set)?;
    ensure::register(&mut set)?;
    registry::register(&mut set)?;
    Ok(set)
}

#[derive(Args)]
struct Settings {
    #[arg(long)]
    config: PathBuf,
    #[arg(long)]
    credentials: bool,
    #[arg(long)]
    github_output: bool,
}

#[derive(Serialize)]
struct RegistrySettings {
    host: String,
    username: String,
    push: bool,
    #[serde(rename = "pull-username")]
    pull_username: String,
}

fn load(path: &Path) -> Result<Config, TaskError> {
    toml::from_str(&fs::read_to_string(path).map_err(TaskError::operation)?)
        .map_err(TaskError::operation)
}
fn username(host: &CiEnvironment, registry: &config::Registry) -> String {
    host.text("PLATFORM_IMAGES_REGISTRY_USERNAME")
        .unwrap_or_else(|| registry.username.clone())
}
fn settings(ctx: &TaskContext, args: Settings) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let cfg = load(&host.root.join(args.config))?;
    let username = username(&host, &cfg.registry);
    let pull_username = if args.credentials {
        username.clone()
    } else {
        String::new()
    };
    report(
        &host,
        &RegistrySettings {
            host: cfg.registry.host,
            username,
            pull_username,
            push: args.credentials,
        },
        args.github_output,
    )
}

fn copy(root: &Path, context: &Path, paths: &[&str]) -> TaskResult {
    for path in paths {
        files::copy(&root.join(path), &context.join(path)).map_err(TaskError::operation)?;
    }
    Ok(())
}

type Arguments = Vec<(String, String)>;
fn with_parent(extra: &toml::Table, parent: &str) -> Result<Arguments, TaskError> {
    if parent.is_empty() {
        return Err(TaskError::operation("The image layer requires --parent"));
    }
    let mut args = Vec::new();
    for (key, value) in extra {
        let value = value
            .as_str()
            .ok_or_else(|| TaskError::operation(format!("Invalid build argument {key}")))?;
        args.push((key.clone(), value.to_owned()));
    }
    if let Some((_, value)) = args.iter_mut().find(|(k, _)| k == "BASE_IMAGE") {
        parent.clone_into(value);
    } else {
        args.push(("BASE_IMAGE".into(), parent.into()));
    }
    Ok(args)
}

async fn layer(
    ctx: &TaskContext,
    a: &Prepare,
    cfg: &Config,
    repo: &Path,
    unit: &Path,
    dest: &Path,
) -> Result<(String, Arguments, bool), TaskError> {
    match a.layer {
        Layer::Base => {
            let flavor =
                cfg.base.flavors.get(&a.flavor).ok_or_else(|| {
                    TaskError::operation(format!("Unknown base flavor {}", a.flavor))
                })?;
            copy(
                &unit.join("images/linux"),
                dest,
                &["Dockerfile", "packages", "bin"],
            )?;
            copy(unit, dest, &["mise", "scripts"])?;
            if !cfg.base.packages.is_empty() {
                fs::write(
                    dest.join("packages/90-config.txt"),
                    format!(
                        "# Extra packages from the consumer config ([base] packages)\n{}\n",
                        cfg.base.packages.join("\n")
                    ),
                )
                .map_err(TaskError::operation)?;
            }
            Ok((
                cfg.base.name.clone(),
                vec![
                    ("BASE_IMAGE".into(), flavor.from.clone()),
                    ("BROWSERS".into(), cfg.base.browsers.join(" ")),
                ],
                false,
            ))
        }
        Layer::Image => {
            let img = &cfg.image;
            let args = with_parent(&img.build_args, &a.parent)?;
            files::copy(&repo.join(&img.dockerfile), &dest.join("Dockerfile"))
                .map_err(TaskError::operation)?;
            inputs::stage(ctx, repo, dest, &img.inputs).await?;
            if img.cargo_fetch {
                inputs::skeleton(ctx, repo, &dest.join("cargo-workspace")).await?;
            }
            Ok((img.name.clone(), args, img.cargo_fetch))
        }
        Layer::VmBase | Layer::VmImage => {
            if !matches!(a.os.as_str(), "windows" | "macos") || a.flavor != "default" {
                return Err(TaskError::operation(
                    "VM layers require --os windows or macos and the default flavor",
                ));
            }
            let vm = cfg.vm.get(&a.os).ok_or_else(|| {
                TaskError::operation(format!("Missing vm.{} configuration", a.os))
            })?;
            if matches!(a.layer, Layer::VmBase) {
                copy(
                    &unit.join("images").join(&a.os),
                    dest,
                    &["Dockerfile", "guest"],
                )?;
                copy(&unit.join("images/vm"), dest, &["bin"])?;
                copy(unit, &dest.join("guest"), &["mise", "scripts"])?;
                Ok((
                    vm.base_name.clone(),
                    vec![("BASE_IMAGE".into(), vm.base_from.clone())],
                    false,
                ))
            } else {
                let args = with_parent(&vm.build_args, &a.parent)?;
                files::copy(&repo.join(&vm.dockerfile), &dest.join("Dockerfile"))
                    .map_err(TaskError::operation)?;
                files::copy(&repo.join(&vm.layer_dir), &dest.join("layer"))
                    .map_err(TaskError::operation)?;
                inputs::stage(
                    ctx,
                    repo,
                    &dest.join("layer"),
                    &vm.inputs.inherit(&cfg.image.inputs),
                )
                .await?;
                Ok((vm.name.clone(), args, false))
            }
        }
    }
}

fn report(host: &CiEnvironment, value: &impl Serialize, github: bool) -> TaskResult {
    if github {
        let path = host
            .get("GITHUB_OUTPUT")
            .filter(|p| !p.is_empty())
            .ok_or_else(|| TaskError::operation("--github-output requires GITHUB_OUTPUT"))?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(TaskError::operation)?;
        let value = serde_json::to_value(value).map_err(TaskError::operation)?;
        for (key, value) in value
            .as_object()
            .ok_or_else(|| TaskError::operation("Expected an output record"))?
        {
            let text = value
                .as_str()
                .map_or_else(|| value.to_string(), ToOwned::to_owned);
            if text.contains(['\n', '\r']) {
                let mut delimiter = String::from("EWE_OUTPUT");
                while text.lines().any(|l| l == delimiter) {
                    delimiter.push('_');
                }
                writeln!(file, "{key}<<{delimiter}\n{text}\n{delimiter}")
                    .map_err(TaskError::operation)?;
            } else {
                writeln!(file, "{key}={text}").map_err(TaskError::operation)?;
            }
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(TaskError::operation)?
    );
    Ok(())
}

async fn prepare(ctx: TaskContext, a: Prepare) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let plan = prepare_plan(&ctx, &a).await?;
    report(&host, &plan, a.github_output)
}

async fn prepare_plan(ctx: &TaskContext, a: &Prepare) -> Result<Plan, TaskError> {
    let host = ctx.require::<CiEnvironment>()?;
    let cfg = load(&host.root.join(&a.config))?;
    let repo = host.root.join(&a.repo);
    let unit = a
        .unit
        .as_ref()
        .map_or_else(|| repo.join("ci/platform-images"), |p| host.root.join(p));
    let out = host.root.join(&a.out);
    fs::create_dir_all(&out).map_err(TaskError::operation)?;
    let out = out.canonicalize().map_err(TaskError::operation)?;
    let stage = tempfile::tempdir_in(&out).map_err(TaskError::operation)?;
    let dest = stage.path().join("context");
    fs::create_dir(&dest).map_err(TaskError::operation)?;
    let (name, args, exclude) = layer(ctx, a, &cfg, &repo, &unit, &dest).await?;
    let lines = files::inputs(&dest, exclude).map_err(TaskError::operation)?;
    let hash = files::hash(
        &lines,
        &args.iter().cloned().collect::<BTreeMap<_, _>>(),
        &a.parent,
    );
    let suffix = if a.flavor == "default" {
        String::new()
    } else {
        format!("-{}", a.flavor)
    };
    let repository = format!(
        "{}/{}/{name}{suffix}",
        cfg.registry.host, cfg.registry.namespace
    );
    let tag = format!("hash-{hash}");
    let context = out.join("context");
    let arguments: Vec<_> = args.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let plan = Plan {
        layer: a.layer,
        flavor: a.flavor.clone(),
        hash,
        reference: format!("{repository}:{tag}"),
        cache_ref: format!("{repository}:buildcache"),
        repository,
        tag,
        dockerfile: context.join("Dockerfile"),
        context,
        platform: "linux/amd64",
        build_args: arguments.join("\n"),
        arguments,
        registry_username: username(&host, &cfg.registry),
        registry_host: cfg.registry.host,
    };
    fs::write(stage.path().join("inputs.txt"), lines.join("\n")).map_err(TaskError::operation)?;
    fs::write(
        stage.path().join("plan.json"),
        serde_json::to_vec_pretty(&plan).map_err(TaskError::operation)?,
    )
    .map_err(TaskError::operation)?;
    crate::staging::publish(
        stage,
        &out,
        ["inputs.txt", "context", "plan.json"].map(|name| (name, out.join(name))),
        false,
    )?;
    Ok(plan)
}
