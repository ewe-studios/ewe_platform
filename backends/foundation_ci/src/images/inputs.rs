//! Collect the locked source, tool, and package inputs for a consumer layer.
use super::{config::Inputs, files};
use crate::workspace::capture;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Read},
    path::Path,
};

#[derive(Serialize)]
struct Entry<'a> {
    path: &'a str,
    url: String,
    sha: String,
}
#[derive(Deserialize)]
struct CargoLock {
    package: Vec<Locked>,
}
#[derive(Deserialize)]
struct Locked {
    name: String,
    version: String,
}

async fn git(ctx: &TaskContext, root: &Path, args: &[&str]) -> Result<String, TaskError> {
    let mut buffer = String::new();
    capture(
        ctx,
        ProcessCommand::new("git")
            .arg("-C")
            .arg(root)
            .args(args.iter().copied()),
    )
    .await?
    .read_to_string(&mut buffer)
    .map_err(TaskError::operation)?;
    Ok(buffer)
}

async fn entry<'a>(ctx: &TaskContext, root: &Path, path: &'a str) -> Result<Entry<'a>, TaskError> {
    files::relative(path)?;
    let tree = git(ctx, root, &["ls-tree", "-z", "HEAD", "--", path]).await?;
    let sha = tree
        .trim_end_matches('\0')
        .split_once('\t')
        .filter(|(_, name)| *name == path)
        .and_then(|(fields, _)| fields.strip_prefix("160000 commit "))
        .ok_or_else(|| TaskError::operation(format!("Missing Git submodule {path}")))?;
    let declarations = git(
        ctx,
        root,
        &[
            "config",
            "-z",
            "-f",
            ".gitmodules",
            "--get-regexp",
            r"^submodule\..*\.path$",
        ],
    )
    .await?;
    let name = declarations
        .split('\0')
        .filter_map(|line| line.split_once('\n'))
        .find(|(_, v)| *v == path)
        .and_then(|(k, _)| k.strip_suffix(".path"))
        .ok_or_else(|| TaskError::operation(format!("Missing submodule declaration for {path}")))?;
    let url = git(
        ctx,
        root,
        &[
            "config",
            "-f",
            ".gitmodules",
            "--get",
            &format!("{name}.url"),
        ],
    )
    .await?;
    if url.trim().is_empty() {
        return Err(TaskError::operation(format!("Missing URL for {path}")));
    }
    Ok(Entry {
        path,
        url: url.trim().into(),
        sha: sha.into(),
    })
}

fn tools(root: &Path, dest: &Path, spec: &Inputs) -> TaskResult {
    let Some(source) = &spec.mise_config else {
        if spec.version_checks.as_ref().is_some_and(|v| !v.is_empty()) {
            return Err(TaskError::operation(
                "Version checks need a mise configuration",
            ));
        }
        return Ok(());
    };
    let cfg: toml::Table = toml::from_str(
        &fs::read_to_string(root.join(files::relative(source)?)).map_err(TaskError::operation)?,
    )
    .map_err(TaskError::operation)?;
    let mut filtered = toml::Table::new();
    filtered.insert(
        "tools".into(),
        cfg.get("tools")
            .cloned()
            .unwrap_or_else(|| toml::Table::new().into()),
    );
    if let Some(settings) = cfg
        .get("settings")
        .filter(|v| v.as_table().is_some_and(|t| !t.is_empty()))
    {
        filtered.insert("settings".into(), settings.clone());
    }
    let directory = dest.join("mise");
    fs::create_dir_all(&directory).map_err(TaskError::operation)?;
    fs::write(
        directory.join("mise.toml"),
        toml::to_string_pretty(&filtered).map_err(TaskError::operation)?,
    )
    .map_err(TaskError::operation)?;
    if let Some(lock) = &spec.mise_lock {
        files::copy(
            &root.join(files::relative(lock)?),
            &directory.join("mise.lock"),
        )
        .map_err(TaskError::operation)?;
    }
    if let Some(checks) = spec.version_checks.as_ref().filter(|v| !v.is_empty()) {
        let lock: CargoLock = toml::from_str(
            &fs::read_to_string(root.join("Cargo.lock")).map_err(TaskError::operation)?,
        )
        .map_err(TaskError::operation)?;
        for check in checks {
            let pin = filtered["tools"]
                .get(&check.tool)
                .and_then(|v| v.as_str().or_else(|| v.get("version")?.as_str()))
                .ok_or_else(|| {
                    TaskError::operation(format!("Missing tool {} in {source}", check.tool))
                })?;
            if !lock
                .package
                .iter()
                .any(|p| p.name == check.cargo_package && p.version == pin)
            {
                return Err(TaskError::operation(format!(
                    "Tool {} version {pin} does not match {} in Cargo.lock",
                    check.tool, check.cargo_package
                )));
            }
        }
    }
    Ok(())
}

pub(super) async fn stage(
    ctx: &TaskContext,
    repo: &Path,
    dest: &Path,
    inputs: &Inputs,
) -> TaskResult {
    fs::create_dir_all(dest).map_err(TaskError::operation)?;
    for file in inputs.files.iter().flatten() {
        let file = files::relative(file)?;
        files::copy(&repo.join(file), &dest.join("files").join(file))
            .map_err(TaskError::operation)?;
    }
    tools(repo, dest, inputs)?;
    let mut subs = Vec::new();
    for path in inputs.submodules.iter().flatten() {
        subs.push(entry(ctx, repo, path).await?);
    }
    let json = serde_json::to_vec_pretty(&BTreeMap::from([("submodules", subs)]))
        .map_err(TaskError::operation)?;
    fs::write(dest.join("submodules.lock.json"), json).map_err(TaskError::operation)
}

pub(super) async fn skeleton(ctx: &TaskContext, repo: &Path, dest: &Path) -> TaskResult {
    let list = capture(
        ctx,
        ProcessCommand::new("git").arg("-C").arg(repo).args([
            "ls-files",
            "-z",
            "--",
            "*Cargo.toml",
            "Cargo.lock",
            ".cargo/config.toml",
            "rust-toolchain.toml",
            "*.rs",
        ]),
    )
    .await?;
    for file in BufReader::new(list).split(0) {
        let bytes = file.map_err(TaskError::operation)?;
        let name = files::relative(std::str::from_utf8(&bytes).map_err(TaskError::operation)?)?;
        let target = dest.join(name);
        if name.extension().is_some_and(|v| v == "rs") {
            let parent = target
                .parent()
                .ok_or_else(|| TaskError::operation("Missing skeleton parent"))?;
            fs::create_dir_all(parent).map_err(TaskError::operation)?;
            fs::File::create(target).map_err(TaskError::operation)?;
        } else {
            files::copy(&repo.join(name), &target).map_err(TaskError::operation)?;
        }
    }
    Ok(())
}
