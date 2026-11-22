//! Bind the isolated starter project to a verified Swift package.
use foundation_tasks::{TaskError, TaskResult};
use regex::{NoExpand, Regex};
use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Deserialize)]
struct LocalPackage {
    swift_manifest: String,
    version: String,
    repositories: Repositories,
}

#[derive(Deserialize)]
struct Repositories {
    repository: String,
}

pub(super) fn local(record: &Path, package: &Path, project: &Path) -> TaskResult {
    let record: LocalPackage =
        serde_json::from_reader(fs::File::open(record).map_err(TaskError::operation)?)
            .map_err(TaskError::operation)?;
    point_project(
        project,
        &record.repositories.repository,
        &record.version,
        Some(package),
    )?;
    fs::write(package.join("Package.swift"), record.swift_manifest).map_err(TaskError::operation)
}

pub(super) fn point_project(
    project: &Path,
    repository: &str,
    version: &str,
    package: Option<&Path>,
) -> TaskResult {
    let url = format!("https://github.com/{}.git", repository);
    let matcher = Regex::new(&format!(
        r#"repositoryURL = "{}";\s*requirement = \{{[^}}]*version = {};[^}}]*\}};"#,
        regex::escape(&url),
        regex::escape(version)
    ))
    .map_err(TaskError::operation)?;
    let text = fs::read_to_string(project).map_err(TaskError::operation)?;
    if matcher.find_iter(&text).count() != 1 {
        return Err(TaskError::operation(
            "Expected one starter package at the release version",
        ));
    }
    let Some(package) = package else {
        return Ok(());
    };
    let replacement = format!(
        "relativePath = {};",
        serde_json::to_string(package).map_err(TaskError::operation)?
    );
    let local = matcher.replace(&text, NoExpand(&replacement)).replace(
        "XCRemoteSwiftPackageReference",
        "XCLocalSwiftPackageReference",
    );
    fs::write(project, local).map_err(TaskError::operation)
}

use super::output::query;
use foundation_tasks::{ProcessCommand, TaskContext};

pub(super) async fn commit(
    ctx: &TaskContext,
    checkout: &Path,
    source: &str,
    manifest: &str,
) -> Result<String, TaskError> {
    let git = || ProcessCommand::new("git").current_dir(checkout);
    ctx.exec(git().args(["checkout", "--quiet", "--detach", source]))
        .await?;
    let path = checkout.join("Package.swift");
    let content = manifest.to_owned();
    let changed = blocking::unblock(move || -> std::io::Result<bool> {
        if fs::read_to_string(&path)? == content {
            return Ok(false);
        }
        fs::write(path, content)?;
        Ok(true)
    })
    .await
    .map_err(TaskError::operation)?;
    if !changed {
        return Ok(source.into());
    }
    ctx.exec(git().args(["add", "--", "Package.swift"])).await?;
    let time = query(ctx, git().args(["show", "-s", "--format=%ct", source])).await?;
    let date = format!(
        "{} +0000",
        std::str::from_utf8(&time)
            .map_err(TaskError::operation)?
            .trim()
    );
    let mut create = git();
    for person in ["AUTHOR", "COMMITTER"] {
        create = create
            .env(format!("GIT_{person}_NAME"), "Native UI Release")
            .env(
                format!("GIT_{person}_EMAIL"),
                "native-ui-release@users.noreply.github.com",
            )
            .env(format!("GIT_{person}_DATE"), &date);
    }
    ctx.exec(create.args([
        "-c",
        "commit.gpgsign=false",
        "-c",
        "core.hooksPath=/dev/null",
        "commit",
        "--quiet",
        "-m",
        "Set the native library checksum for this release",
    ]))
    .await?;
    let head = query(ctx, git().args(["rev-parse", "HEAD"])).await?;
    Ok(std::str::from_utf8(&head)
        .map_err(TaskError::operation)?
        .trim()
        .into())
}

pub(super) async fn release(
    ctx: &TaskContext,
    root: &Path,
    source: &str,
    url: &str,
    checksum: &str,
) -> Result<(String, String), TaskError> {
    let path = root.join("Package.swift");
    let original = blocking::unblock(move || fs::read_to_string(path))
        .await
        .map_err(TaskError::operation)?;
    let pattern = Regex::new(&format!(
        r#"(url:\s*"{}",\s*checksum:\s*")[a-f0-9]{{64}}(")"#,
        regex::escape(url)
    ))
    .map_err(TaskError::operation)?;
    if pattern.find_iter(&original).count() != 1 {
        return Err(TaskError::operation(
            "Expected one checksum for the release URL",
        ));
    }
    let manifest = pattern
        .replace(&original, |m: &regex::Captures<'_>| {
            format!("{}{checksum}{}", &m[1], &m[2])
        })
        .into_owned();
    if manifest == original {
        return Ok((source.into(), manifest));
    }
    let stage = blocking::unblock(tempfile::tempdir)
        .await
        .map_err(TaskError::operation)?;
    ctx.exec(
        ProcessCommand::new("git")
            .current_dir(root)
            .args(["clone", "--quiet", "--no-checkout"])
            .arg(root)
            .arg(stage.path()),
    )
    .await?;
    let revision = commit(ctx, stage.path(), source, &manifest).await?;
    Ok((revision, manifest))
}
