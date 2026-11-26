//! Install provider functions in Cargo build scripts.
mod manifest;
mod script;
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
use toml_edit::DocumentMut;

/// A provider function with its Cargo dependency and required features.
#[derive(Clone, Debug)]
pub struct Capability<'a> {
    /// A stable capability name. Use letters, digits, and hyphens.
    pub name: &'a str,
    /// The package name from the provider manifest.
    pub package: &'a str,
    /// The exact version that supplies the function.
    pub version: &'a str,
    /// An optional local provider directory. The installed manifest uses a relative path when possible.
    pub path: Option<PathBuf>,
    /// The Cargo features that enable the build hook.
    pub features: &'a [&'a str],
    /// The path to a zero-argument function within the provider crate.
    pub function: &'a str,
    /// Set this field when the hook returns a result.
    pub fallible: bool,
}

fn invalid(message: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

fn read(path: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

fn replace(path: &Path, previous: Option<&str>, next: &str) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("Missing parent directory"))?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    if previous.is_some() {
        staged
            .as_file()
            .set_permissions(fs::metadata(path)?.permissions())?;
    }
    staged.write_all(next.as_bytes())?;
    staged.as_file().sync_all()?;
    if read(path)?.as_deref() != previous {
        return Err(invalid(format!("Concurrent change to {}", path.display())));
    }
    if previous.is_none() {
        staged.persist_noclobber(path)?;
    } else {
        staged.persist(path)?;
    }
    Ok(())
}

/// Install a provider, or verify the current setup in check mode.
///
/// # Errors
/// Reports conflicting declarations, concurrent changes, or filesystem failures.
pub fn install(path: &Path, provider: &Capability<'_>, check: bool) -> io::Result<()> {
    let path = path.canonicalize()?;
    let dir = path
        .parent()
        .ok_or_else(|| invalid("Expected a package manifest"))?;
    if provider.name.is_empty()
        || !provider
            .name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(invalid("Invalid capability name"));
    }
    // A stable lock serializes installers across replacement of either destination.
    let _lock = if check {
        None
    } else {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut hash = DefaultHasher::new();
        path.hash(&mut hash);
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(std::env::temp_dir().join(format!("ewe-build-hook-{:x}.lock", hash.finish())))?;
        lock.lock()?;
        Some(lock)
    };
    let original = fs::read_to_string(&path)?;
    let mut doc = original.parse::<DocumentMut>().map_err(invalid)?;
    let pkg = doc
        .get("package")
        .ok_or_else(|| invalid("Missing package table"))?;
    let build = match pkg.get("build") {
        None => "build.rs",
        Some(i) if i.as_bool() == Some(true) => "build.rs",
        Some(i) => i.as_str().ok_or_else(|| {
            invalid(format!(
                "{}: build script disabled or invalid",
                path.display()
            ))
        })?,
    };
    let script_path = dir.join(build);
    let script_path = if script_path.exists() {
        script_path.canonicalize()?
    } else {
        script_path
    };
    let script_original = read(&script_path)?;
    let alias = manifest::edit(dir, &mut doc, provider)
        .map_err(|e| invalid(format!("{}: {e}", path.display())))?;
    let script_next = script::edit(
        script_original.as_deref().unwrap_or("fn main() {\n}\n"),
        &alias,
        provider,
    )
    .map_err(|e| invalid(format!("{}: {e}", script_path.display())))?;
    let manifest_next = doc.to_string();
    let manifest_dirty = manifest_next != original;
    let script_dirty = script_original.as_deref() != Some(&script_next);
    if check && (manifest_dirty || script_dirty) {
        return Err(invalid(format!(
            "Missing {} setup in {} or {}",
            provider.name,
            path.display(),
            script_path.display()
        )));
    }
    if manifest_dirty {
        replace(&path, Some(&original), &manifest_next)?;
    }
    if !script_dirty {
        return Ok(());
    }
    if let Err(error) = replace(&script_path, script_original.as_deref(), &script_next) {
        if manifest_dirty {
            replace(&path, Some(&manifest_next), &original).map_err(|rollback| {
                invalid(format!(
                    "{error}; rollback of {} failed: {rollback}",
                    path.display()
                ))
            })?;
        }
        return Err(error);
    }
    Ok(())
}

/// Export an installation task for the capabilities supplied by the calling host.
///
/// # Errors
/// Returns a task registration error for an invalid task definition.
pub fn task_set(providers: Vec<Capability<'static>>) -> Result<crate::TaskSet, crate::TaskError> {
    use clap::Args;
    use std::sync::Arc;
    #[derive(Args)]
    struct InstallArgs {
        /// Path to the consumer's Cargo manifest.
        #[arg(long, default_value = "Cargo.toml")]
        manifest_path: PathBuf,
        /// Registered capability to install.
        capability: String,
        /// Verify setup and leave both files unchanged.
        #[arg(long)]
        check: bool,
    }
    let providers = Arc::new(providers);
    let mut tasks = crate::TaskSet::new("foundation_tasks").with_namespace("build");
    tasks.add(
        "install-hook",
        "Install a Cargo build hook",
        move |_, args: InstallArgs| {
            let providers = Arc::clone(&providers);
            async move {
                let provider = providers
                    .iter()
                    .find(|p| p.name == args.capability)
                    .ok_or_else(|| {
                        crate::TaskError::operation(format!(
                            "Unknown build capability {}",
                            args.capability
                        ))
                    })?;
                let provider = provider.clone();
                blocking::unblock(move || install(&args.manifest_path, &provider, args.check))
                    .await
                    .map_err(crate::TaskError::operation)
            }
        },
    )?;
    Ok(tasks)
}
