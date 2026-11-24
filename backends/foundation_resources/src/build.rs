//! Resolve application resource declarations for one packaging target.
use crate::ResourceName;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs, io,
    path::{Component, Path, PathBuf},
};

/// The runtime that consumes a staged resource set.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    Web,
    Apple,
    Android,
}

/// Paths that the caller passes to its packager and build system.
#[derive(Debug)]
pub struct Staged {
    pub directory: PathBuf,
    pub names: Vec<String>,
    pub inputs: BTreeSet<PathBuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    #[serde(default)]
    resource: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    name: String,
    path: Option<PathBuf>,
    generated: Option<PathBuf>,
    #[serde(default)]
    targets: Vec<Target>,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
fn checked_source(root: &Path, relative: &Path) -> io::Result<PathBuf> {
    if relative.as_os_str().is_empty()
        || !relative
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(invalid(format!(
            "Expected a package-relative resource source: {}",
            relative.display()
        )));
    }
    let full = root.join(relative);
    let path = full.canonicalize().map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("Resource source {}: {error}", full.display()),
        )
    })?;
    if !path.starts_with(root.canonicalize()?) {
        return Err(invalid(format!(
            "Resource source escapes its directory: {}",
            full.display()
        )));
    }
    Ok(path)
}

#[derive(Default)]
struct Inputs {
    files: BTreeMap<String, PathBuf>,
    paths: BTreeSet<PathBuf>,
    portable: BTreeMap<String, (String, bool)>,
}
impl Inputs {
    fn add(&mut self, name: &str, path: &Path) -> io::Result<()> {
        ResourceName::new(name).map_err(invalid_name)?;
        self.paths.insert(path.to_owned());
        let info = fs::symlink_metadata(path)?;
        if info.is_symlink() {
            return Err(invalid(format!(
                "Resource {name} contains a symbolic link: {}",
                path.display()
            )));
        }
        let mut prefix = String::new();
        for part in name.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            let is_dir = prefix != name || info.is_dir();
            let key = prefix.to_lowercase();
            if let Some((previous, was_dir)) = self.portable.get(&key) {
                if previous != &prefix || !was_dir || !is_dir {
                    return Err(invalid(format!(
                        "Resource {name} conflicts with {previous}"
                    )));
                }
            } else {
                self.portable.insert(key, (prefix.clone(), is_dir));
            }
        }
        if info.is_dir() {
            for item in fs::read_dir(path)? {
                let item = item?;
                let part = item
                    .file_name()
                    .into_string()
                    .map_err(|_| invalid(format!("Resource {name} has a non-Unicode file name")))?;
                self.add(&format!("{name}/{part}"), &item.path())?;
            }
        } else if info.is_file() {
            self.files.insert(name.to_owned(), path.to_owned());
        } else {
            return Err(invalid(format!(
                "Unsupported resource {name}: {}",
                path.display()
            )));
        }
        Ok(())
    }
}
fn invalid_name(error: crate::InvalidName) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error)
}

const INDEX: &str = ".ewe-resource-index";
fn preserve_user_files(
    root: &Path,
    relative: &Path,
    stage: &Path,
    managed: &BTreeSet<String>,
    expected: &Inputs,
) -> io::Result<()> {
    for item in fs::read_dir(root.join(relative))? {
        let item = item?;
        let name = relative.join(item.file_name());
        if name == Path::new(INDEX) {
            continue;
        }
        let logical = name
            .to_str()
            .ok_or_else(|| invalid("Non-Unicode destination path"))?
            .replace('\\', "/");
        let file_type = item.file_type()?;
        if managed.contains(&logical) && file_type.is_file() {
            continue;
        }
        let output = stage.join(&name);
        if file_type.is_dir() {
            let owned = managed.contains(&format!("{logical}/"));
            if !owned {
                if let Some((spelling, is_directory)) =
                    expected.portable.get(&logical.to_lowercase())
                {
                    if spelling != &logical || !is_directory {
                        return Err(invalid(format!(
                            "Resource {spelling} conflicts with user directory {logical}"
                        )));
                    }
                }
                fs::create_dir_all(&output)?;
            }
            preserve_user_files(root, &name, stage, managed, expected)?;
        } else {
            if expected.portable.contains_key(&logical.to_lowercase()) {
                return Err(invalid(format!(
                    "Resource {logical} would replace a user file"
                )));
            }
            if !file_type.is_file() {
                return Err(invalid(format!(
                    "Cannot replace destination containing special file {logical}"
                )));
            }
            fs::create_dir_all(output.parent().ok_or_else(|| invalid(&logical))?)?;
            fs::copy(item.path(), output)?;
        }
    }
    Ok(())
}

/// Stage a complete set in the destination. Preserve other files in that directory.
///
/// # Errors
/// Returns an error for invalid declarations, conflicting names, or failed file operations.
pub fn stage(
    manifest: &Path,
    generated: &Path,
    target: Target,
    destination: &Path,
) -> io::Result<Staged> {
    stage_in(manifest, generated, target, destination, None, &[])
}

fn resolve_destination(destination: &Path) -> io::Result<PathBuf> {
    let destination_path = std::path::absolute(destination)?;
    let existing = destination_path
        .ancestors()
        .find(|path| path.exists())
        .ok_or_else(|| {
            invalid(format!(
                "Cannot resolve destination {}",
                destination.display()
            ))
        })?;
    Ok(existing.canonicalize()?.join(
        destination_path
            .strip_prefix(existing)
            .map_err(|e| invalid(e.to_string()))?,
    ))
}

fn stage_in(
    manifest: &Path,
    generated: &Path,
    target: Target,
    destination: &Path,
    payload: Option<&ResourceName>,
    outputs: &[(String, PathBuf)],
) -> io::Result<Staged> {
    let parent = manifest
        .parent()
        .ok_or_else(|| invalid("Resource manifest needs a directory"))?;
    let declarations: Manifest = toml::from_str(&fs::read_to_string(manifest)?)
        .map_err(|e| invalid(format!("{}: {e}", manifest.display())))?;
    let resolved_destination = resolve_destination(destination)?;
    if manifest.canonicalize()?.starts_with(&resolved_destination) {
        return Err(invalid(format!(
            "Resource destination {} contains the manifest",
            destination.display()
        )));
    }
    let mut inputs = Inputs::default();
    inputs.paths.insert(manifest.to_owned());
    let mut names = BTreeSet::new();
    for (name, path) in outputs {
        let src = checked_source(generated, path)?;
        inputs.add(name, &src)?;
        names.insert(name.clone());
    }

    for entry in declarations.resource {
        if !entry.targets.is_empty() && !entry.targets.contains(&target) {
            continue;
        }
        if !names.insert(entry.name.clone()) {
            return Err(invalid(format!("Duplicate resource {}", entry.name)));
        }
        let src = match (entry.path, entry.generated) {
            (Some(path), None) => checked_source(parent, &path),
            (None, Some(path)) => checked_source(generated, &path),
            _ => {
                return Err(invalid(format!(
                    "Resource {} needs one path or generated input",
                    entry.name
                )))
            }
        }
        .map_err(|err| {
            io::Error::new(
                err.kind(),
                format!(
                    "Resource {} for {}: {err}",
                    entry.name,
                    destination.display()
                ),
            )
        })?;
        if src.starts_with(&resolved_destination) || resolved_destination.starts_with(&src) {
            return Err(invalid(format!(
                "Resource {} overlaps destination {}",
                entry.name,
                destination.display()
            )));
        }
        let name = payload.map_or_else(
            || entry.name.clone(),
            |base| format!("{}/{}", base.as_str(), entry.name),
        );
        inputs.add(&name, &src).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "Resource {} for {}: {error}",
                    entry.name,
                    destination.display()
                ),
            )
        })?;
    }
    if let Some(payload) = payload {
        inputs
            .portable
            .entry(payload.as_str().to_lowercase())
            .or_insert_with(|| (payload.as_str().into(), true));
    }
    publish(&inputs, destination).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "Cannot publish resources to {}: {error}",
                destination.display()
            ),
        )
    })?;
    let prefix = payload.map_or(String::new(), |base| format!("{}/", base.as_str()));
    Ok(Staged {
        directory: destination.join(&prefix),
        names: inputs
            .files
            .into_keys()
            .map(|name| name[prefix.len()..].to_owned())
            .collect(),
        inputs: inputs.paths,
    })
}

pub(crate) fn same_file(first: &Path, second: &Path) -> io::Result<bool> {
    use std::io::BufRead;
    let meta = match fs::metadata(second) {
        Ok(meta) => meta,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if !meta.is_file() || fs::metadata(first)?.len() != meta.len() {
        return Ok(false);
    }
    let mut a = io::BufReader::new(fs::File::open(first)?);
    let mut b = io::BufReader::new(fs::File::open(second)?);
    loop {
        let left = a.fill_buf()?;
        let right = b.fill_buf()?;
        let count = left.len().min(right.len());
        if count == 0 {
            return Ok(left.is_empty() && right.is_empty());
        }
        if left[..count] != right[..count] {
            return Ok(false);
        }
        a.consume(count);
        b.consume(count);
    }
}

fn recover_previous(dest: &Path, backup: &Path) -> io::Result<()> {
    if backup.exists() {
        if !backup.join(INDEX).is_file() {
            return Err(invalid(format!(
                "Resource backup path contains user files: {}",
                backup.display()
            )));
        }
        if dest.exists() {
            fs::remove_dir_all(backup)?;
        } else {
            fs::rename(backup, dest)?;
        }
    }
    Ok(())
}

fn publish(inputs: &Inputs, dest: &Path) -> io::Result<()> {
    let parent = dest
        .parent()
        .ok_or_else(|| invalid("Resource destination needs a parent"))?;
    let name = dest
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| invalid("Invalid destination name"))?;
    fs::create_dir_all(parent)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(parent.join(format!(".{name}.ewe-resources.lock")))?;
    lock.lock()?;
    if fs::symlink_metadata(dest).is_ok_and(|meta| meta.is_symlink()) {
        return Err(invalid(format!(
            "Resource destination is a symbolic link: {}",
            dest.display()
        )));
    }
    let backup = parent.join(format!(".{name}.ewe-resources.previous"));
    recover_previous(dest, &backup)?;
    let stage = tempfile::tempdir_in(parent)?;
    if dest.exists() {
        let list = match fs::read_to_string(dest.join(INDEX)) {
            Ok(list) => list,
            Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
            Err(err) => return Err(err),
        };
        let managed = list.lines().map(str::to_owned).collect();
        preserve_user_files(dest, Path::new(""), stage.path(), &managed, inputs)?;
    }
    let mut index = BTreeSet::new();
    for (logical, is_directory) in inputs.portable.values() {
        if *is_directory {
            let path = stage.path().join(logical);
            if !path.exists() {
                fs::create_dir_all(path)?;
                index.insert(format!("{logical}/"));
            }
        }
    }
    for (name, source) in &inputs.files {
        index.insert(name.clone());
        let file = stage.path().join(name);
        fs::create_dir_all(file.parent().ok_or_else(|| invalid(name))?)?;
        fs::copy(source, &file).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("Resource {name} for {}: {e}", dest.display()),
            )
        })?;
    }
    fs::write(
        stage.path().join(INDEX),
        index.into_iter().collect::<Vec<_>>().join("\n"),
    )?;
    let mut unchanged = same_file(&stage.path().join(INDEX), &dest.join(INDEX))?;
    if unchanged {
        for (name, is_directory) in inputs.portable.values() {
            let path = dest.join(name);
            let matches = if *is_directory {
                path.is_dir()
            } else {
                same_file(&stage.path().join(name), &path)?
            };
            if !matches {
                unchanged = false;
                break;
            }
        }
    }
    if unchanged {
        return stage.close();
    }
    #[cfg(any(target_vendor = "apple", target_os = "linux", target_os = "android"))]
    if dest.exists() {
        use rustix::fs::{renameat_with, RenameFlags, CWD};
        renameat_with(CWD, stage.path(), CWD, dest, RenameFlags::EXCHANGE)?;
        return stage.close();
    }
    if dest.exists() {
        fs::rename(dest, &backup)?;
    }
    if let Err(error) = fs::rename(stage.path(), dest) {
        if backup.exists() {
            fs::rename(&backup, dest)?;
        }
        return Err(error);
    }
    if backup.exists() {
        fs::remove_dir_all(backup)?;
    }
    Ok(())
}

/// Stage the current package after its build script finishes generating inputs.
///
/// # Errors
/// Returns an error when Cargo variables or resource inputs are missing.
pub fn stage_from_cargo() -> io::Result<Staged> {
    stage_outputs(&[])
}

pub(crate) fn stage_outputs(outputs: &[(String, PathBuf)]) -> io::Result<Staged> {
    for key in [
        "EWE_RESOURCE_TARGET",
        "EWE_RESOURCE_DEST",
        "EWE_RESOURCE_PAYLOAD",
        "CARGO_TARGET_DIR",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    let var = |key| {
        env::var_os(key)
            .map(PathBuf::from)
            .ok_or_else(|| invalid(format!("Cargo did not set {key}")))
    };
    let package = var("CARGO_MANIFEST_DIR")?;
    let generated = var("OUT_DIR")?;
    let target = match env::var("EWE_RESOURCE_TARGET") {
        Ok(value) => match value.as_str() {
            "apple" => Target::Apple,
            "android" => Target::Android,
            "web" => Target::Web,
            other => return Err(invalid(format!("Unknown resource target {other}"))),
        },
        Err(env::VarError::NotPresent) => match env::var("CARGO_CFG_TARGET_OS")
            .map_err(|e| invalid(e.to_string()))?
            .as_str()
        {
            "ios" | "tvos" | "watchos" | "visionos" => Target::Apple,
            "android" => Target::Android,
            _ => Target::Web,
        },
        Err(error) => return Err(invalid(error.to_string())),
    };
    let dest =
        env::var_os("EWE_RESOURCE_DEST").map_or_else(|| generated.join("resources"), PathBuf::from);
    let payload = match env::var("EWE_RESOURCE_PAYLOAD") {
        Ok(name) => Some(ResourceName::new(&name).map_err(invalid_name)?),
        Err(env::VarError::NotPresent) => None,
        Err(error) => return Err(invalid(error.to_string())),
    };
    if payload.is_some() && env::var_os("EWE_RESOURCE_DEST").is_none() {
        return Err(invalid("A package payload needs EWE_RESOURCE_DEST"));
    }
    let staged = stage_in(
        &package.join("resources.toml"),
        &generated,
        target,
        &dest,
        payload.as_ref(),
        outputs,
    )?;
    if target == Target::Web && env::var_os("EWE_RESOURCE_DEST").is_none() {
        let profile = generated
            .ancestors()
            .find(|path| path.file_name() == Some(std::ffi::OsStr::new("build")))
            .and_then(Path::parent)
            .ok_or_else(|| {
                invalid(format!(
                    "Cannot locate the profile directory from {}",
                    generated.display()
                ))
            })?;
        let public = profile.join("public").join(var("CARGO_PKG_NAME")?);
        stage_in(
            &package.join("resources.toml"),
            &generated,
            target,
            &public,
            None,
            outputs,
        )?;
    }
    println!("cargo:rerun-if-changed={}", dest.join(INDEX).display());
    for name in &staged.names {
        println!(
            "cargo:rerun-if-changed={}",
            staged.directory.join(name).display()
        );
    }
    write_provider(&generated, &staged)?;
    let names = generated.join("ewe-resource-names");
    let content = staged.names.join("\n");
    let unchanged = match fs::metadata(&names) {
        Ok(meta) => meta.len() == content.len() as u64 && fs::read(&names)? == content.as_bytes(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => return Err(error),
    };
    if !unchanged {
        fs::write(&names, content)?;
    }
    println!("cargo:rustc-env=EWE_RESOURCE_NAMES={}", names.display());
    for path in &staged.inputs {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!(
        "cargo:rustc-env=EWE_STAGED_RESOURCES={}",
        staged.directory.display()
    );
    Ok(staged)
}

/// Emit a sorted lookup table. Cargo selects the consuming crate's build mode.
fn write_provider(generated: &Path, staged: &Staged) -> io::Result<()> {
    use std::fmt::Write;
    let mut names: Vec<_> = staged.names.iter().collect();
    names.sort_unstable();
    let mut table = String::from("&[\n");
    for name in names {
        let input = staged.directory.join(name);
        let input = input
            .to_str()
            .ok_or_else(|| invalid("Non-Unicode embedded resource path"))?;
        writeln!(table,"({name:?}, {{ #[cfg(debug_assertions)] {{ None }} #[cfg(not(debug_assertions))] {{ Some(include_bytes!({input:?}) as &'static [u8]) }} }}),")
            .map_err(io::Error::other)?;
    }
    table.push_str("]\n");
    let path = generated.join("ewe-packaged-resources.rs");
    match fs::read(&path) {
        Ok(bytes) if bytes == table.as_bytes() => Ok(()),
        Ok(_) => fs::write(path, table),
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::write(path, table),
        Err(error) => Err(error),
    }
}
