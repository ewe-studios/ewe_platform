//! Check publish cycles, package eligible workspace members, and build the native bindings from their archive.
use crate::{workspace::capture, CiEnvironment};
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult, TaskSet};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    path::{Path, PathBuf},
};

#[derive(Args)]
struct PackageArgs {
    /// Stop after checking for versioned development dependency cycles.
    #[arg(long)]
    check_cycles: bool,
}
#[derive(Deserialize)]
struct Packages {
    packages: Vec<Member>,
    target_directory: PathBuf,
}
#[derive(Deserialize)]
struct Member {
    name: String,
    version: String,
    manifest_path: PathBuf,
    publish: Option<Vec<String>>,
    dependencies: Vec<Dependency>,
}
#[derive(Deserialize)]
struct Dependency {
    name: String,
    kind: Option<String>,
    path: Option<PathBuf>,
    req: String,
    optional: bool,
}
impl Member {
    fn publishable(&self) -> bool {
        self.publish
            .as_ref()
            .is_none_or(|registries| !registries.is_empty())
    }
}

fn path_to<'a>(
    members: &BTreeMap<&'a str, &'a Member>,
    start: &'a str,
    goal: &str,
) -> Option<Vec<&'a str>> {
    let mut previous = BTreeMap::from([(start, None)]);
    let mut queue = VecDeque::from([start]);
    while let Some(current) = queue.pop_front() {
        if current == goal {
            let mut path = vec![current];
            let mut last = current;
            while let Some(parent) = previous[last] {
                path.push(parent);
                last = parent;
            }
            path.reverse();
            return Some(path);
        }
        for dep in &members.get(current)?.dependencies {
            let next = dep.name.as_str();
            if dep.kind.as_deref() == Some("dev") || dep.optional || !members.contains_key(next) {
                continue;
            }
            if let std::collections::btree_map::Entry::Vacant(entry) = previous.entry(next) {
                entry.insert(Some(current));
                queue.push_back(next);
            }
        }
    }
    None
}

fn check_cycles(packages: &[Member]) -> TaskResult {
    let members: BTreeMap<_, _> = packages.iter().map(|p| (p.name.as_str(), p)).collect();
    let mut cycles = 0;
    for member in packages.iter().filter(|p| p.publishable()) {
        for dep in &member.dependencies {
            if dep.kind.as_deref() != Some("dev")
                || dep.path.is_none()
                || dep.req == "*"
                || !members.contains_key(dep.name.as_str())
            {
                continue;
            }
            if let Some(path) = path_to(&members, &dep.name, &member.name) {
                eprintln!("FAILED: publish cycle: {} -(dev)-> {}\n  remove `version` from the `{}` dev-dependency of {}",member.name,path.join(" -> "),dep.name,member.name);
                cycles += 1;
            }
        }
    }
    if cycles == 0 {
        Ok(())
    } else {
        Err(TaskError::operation(format!(
            "{cycles} dev-dependency publish cycle(s) found"
        )))
    }
}

fn patch_table(packages: &[Member], destination: &Path) -> TaskResult {
    #[derive(serde::Serialize)]
    struct Local<'a> {
        path: &'a Path,
    }
    let mut members = BTreeMap::new();
    for p in packages {
        let path = p
            .manifest_path
            .parent()
            .ok_or_else(|| TaskError::operation("Missing package directory"))?;
        members.insert(&p.name, Local { path });
    }
    let patch = BTreeMap::from([("patch", BTreeMap::from([("crates-io", members)]))]);
    fs::write(
        destination,
        toml::to_string(&patch).map_err(TaskError::operation)?,
    )
    .map_err(TaskError::operation)
}

async fn isolated(ctx: &TaskContext, member: &Member, target: &Path) -> TaskResult {
    let name = format!("{}-{}", member.name, member.version);
    let path = target.join("package").join(format!("{name}.crate"));
    let temp = tempfile::tempdir().map_err(TaskError::operation)?;
    ctx.exec(
        ProcessCommand::new("tar")
            .arg("-xf")
            .arg(path)
            .arg("-C")
            .arg(temp.path()),
    )
    .await?;
    let mut cargo = ProcessCommand::new("cargo")
        .args(["check", "--lib"])
        .current_dir(temp.path().join(name))
        .env("CARGO_TARGET_DIR", temp.path().join("target"));
    for key in [
        "LLAMA_DIR",
        "TOOLS_DIR",
        "EMSDK_DIR",
        "LLAMA_BINDINGS_DIR",
        "LLAMA_SYMLINK_DIR",
        "LLAMA_SUBMODULE_PATH",
        "DAWN_DIR",
        "WHISPER_DIR",
        "CMAKE_PREFIX_PATH",
        "EWE_PLATFORM",
        "EWE_PLATFORM_DIR",
        "CARGO_PROFILE_DEV_CODEGEN_BACKEND",
        "CARGO_PROFILE_UAT_CODEGEN_BACKEND",
        "CARGO_UNSTABLE_CODEGEN_BACKEND",
    ] {
        cargo = cargo.env_remove(key);
    }
    ctx.exec(cargo).await?;
    temp.close().map_err(TaskError::operation)
}

pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add(
        "package-workspace",
        "Verify workspace package archives",
        package,
    )?;
    set.expose("package-workspace", &["package-workspace"])
}

async fn package(ctx: TaskContext, args: PackageArgs) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let metadata = capture(
        &ctx,
        ProcessCommand::new("cargo").current_dir(&host.root).args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
        ]),
    )
    .await?;
    let plan: Packages =
        serde_json::from_reader(std::io::BufReader::new(metadata)).map_err(TaskError::operation)?;
    let members: Vec<_> = plan.packages.iter().filter(|p| p.publishable()).collect();
    if members.is_empty() {
        return Err(TaskError::operation("no publishable workspace crates"));
    }
    check_cycles(&plan.packages)?;
    if args.check_cycles {
        return Ok(());
    }
    let temp = tempfile::tempdir().map_err(TaskError::operation)?;
    let config = temp.path().join("patch.toml");
    patch_table(&plan.packages, &config)?;
    let mut failed = Vec::new();
    for p in &members {
        println!("packaging {} {}", p.name, p.version);
        let result = ctx
            .exec(
                ProcessCommand::new("cargo")
                    .current_dir(&host.root)
                    .args(["package", "--package", &p.name, "--no-verify", "--config"])
                    .arg(&config),
            )
            .await;
        match result {
            Ok(()) => {}
            Err(TaskError::Cancelled) => return Err(TaskError::Cancelled),
            Err(error) => {
                eprintln!("FAILED: {}: {error}", p.name);
                failed.push(p.name.as_str());
            }
        }
    }
    if let Some(p) = members
        .iter()
        .find(|p| p.name == "infrastructure_llama_bindings" && !failed.contains(&p.name.as_str()))
    {
        println!(
            "building {} {} from its tarball, with no build env",
            p.name, p.version
        );
        match isolated(&ctx, p, &plan.target_directory).await {
            Ok(()) => {}
            Err(TaskError::Cancelled) => return Err(TaskError::Cancelled),
            Err(error) => {
                eprintln!("FAILED: {} (isolated tarball build): {error}", p.name);
                failed.push(&p.name);
            }
        }
    }
    temp.close().map_err(TaskError::operation)?;
    if failed.is_empty() {
        println!("packaged {} publishable crates", members.len());
        Ok(())
    } else {
        Err(TaskError::operation(format!(
            "{} of {} crates failed to package: {}",
            failed.len(),
            members.len(),
            failed.join(", ")
        )))
    }
}
