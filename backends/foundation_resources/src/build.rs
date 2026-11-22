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
    invalid(error.to_string())
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
    let parent = manifest
        .parent()
        .ok_or_else(|| invalid("Resource manifest needs a directory"))?;
    let declarations: Manifest = toml::from_str(&fs::read_to_string(manifest)?)
        .map_err(|e| invalid(format!("{}: {e}", manifest.display())))?;
    let mut inputs = Inputs::default();
    inputs.paths.insert(manifest.to_owned());
    let mut names = BTreeSet::new();
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
        inputs.add(&entry.name, &src)?;
    }
    publish(&inputs, destination)?;
    Ok(Staged {
        directory: destination.to_owned(),
        names: inputs.files.into_keys().collect(),
        inputs: inputs.paths,
    })
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
    if backup.exists() {
        if !backup.join(INDEX).is_file() {
            return Err(invalid(format!(
                "Resource backup path contains user files: {}",
                backup.display()
            )));
        }
        if dest.exists() {
            fs::remove_dir_all(&backup)?;
        } else {
            fs::rename(&backup, dest)?;
        }
    }
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
    for key in [
        "EWE_RESOURCE_TARGET",
        "EWE_RESOURCE_DEST",
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
    let target = match env::var("EWE_RESOURCE_TARGET")
        .ok()
        .as_deref()
        .unwrap_or(&env::var("CARGO_CFG_TARGET_OS").unwrap_or_default())
    {
        "apple" | "ios" => "apple",
        "android" => "android",
        "web" | "macos" | "linux" | "windows" | "unknown" => "web",
        other => return Err(invalid(format!("Unknown resource target {other}"))),
    };
    let target = match target {
        "apple" => Target::Apple,
        "android" => Target::Android,
        _ => Target::Web,
    };
    let dest =
        env::var_os("EWE_RESOURCE_DEST").map_or_else(|| generated.join("resources"), PathBuf::from);
    let staged = stage(&package.join("resources.toml"), &generated, target, &dest)?;
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
        stage(&package.join("resources.toml"), &generated, target, &public)?;
    }
    println!(
        "cargo:rerun-if-changed={}",
        staged.directory.join(INDEX).display()
    );
    for name in &staged.names {
        println!(
            "cargo:rerun-if-changed={}",
            staged.directory.join(name).display()
        );
    }
    for path in &staged.inputs {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!(
        "cargo:rustc-env=EWE_STAGED_RESOURCES={}",
        staged.directory.display()
    );
    Ok(staged)
}
