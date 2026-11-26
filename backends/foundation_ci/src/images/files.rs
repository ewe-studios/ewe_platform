//! Stage build context files and hash their paths, contents, and executable modes.
use foundation_tasks::TaskError;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Component, Path},
};

pub(super) fn relative(name: &str) -> Result<&Path, TaskError> {
    let path = Path::new(name);
    if name.is_empty()
        || !path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(TaskError::operation(format!(
            "Expected a relative input path: {name}"
        )));
    }
    Ok(path)
}

pub(super) fn copy(src: &Path, dst: &Path) -> io::Result<()> {
    if fs::symlink_metadata(src)?.is_dir() {
        let parent = dst
            .parent()
            .ok_or_else(|| io::Error::other("Missing context parent"))?;
        fs::create_dir_all(parent)?;
        if parent.canonicalize()?.starts_with(src.canonicalize()?) {
            return Err(io::Error::other(
                "The build context is inside an image input directory",
            ));
        }
    }
    copy_entry(src, dst)
}
fn copy_entry(src: &Path, dst: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(src)?;
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    if metadata.file_type().is_symlink() {
        let value = src.read_link()?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(value, dst)?;
        #[cfg(windows)]
        if src.is_dir() {
            std::os::windows::fs::symlink_dir(value, dst)?;
        } else {
            std::os::windows::fs::symlink_file(value, dst)?;
        }
    } else if metadata.is_dir() {
        fs::create_dir_all(dst)?;
        for item in fs::read_dir(src)? {
            let item = item?;
            copy_entry(&item.path(), &dst.join(item.file_name()))?;
        }
        fs::set_permissions(dst, metadata.permissions())?;
    } else if metadata.is_file() {
        fs::copy(src, dst)?;
    } else {
        return Err(io::Error::other(format!(
            "Unsupported input: {}",
            src.display()
        )));
    }
    Ok(())
}

fn record(
    base: &Path,
    dir: &Path,
    exclude_skeleton: bool,
    lines: &mut BTreeMap<String, String>,
) -> io::Result<()> {
    for item in fs::read_dir(dir)? {
        let item = item?;
        let path = item.path();
        let rel = path.strip_prefix(base).map_err(io::Error::other)?;
        if exclude_skeleton && rel.starts_with("cargo-workspace") {
            continue;
        }
        let key = rel
            .to_str()
            .ok_or_else(|| io::Error::other("Non-Unicode image path"))?
            .replace(std::path::MAIN_SEPARATOR, "/");
        let ty = item.file_type()?;
        if ty.is_dir() {
            record(base, &path, exclude_skeleton, lines)?;
        } else if ty.is_symlink() {
            lines.insert(
                key.clone(),
                format!("{key}\tlink\t{}", path.read_link()?.display()),
            );
        } else {
            #[cfg(unix)]
            let executable = {
                use std::os::unix::fs::PermissionsExt;
                item.metadata()?.permissions().mode() & 0o111 != 0
            };
            #[cfg(not(unix))]
            let executable = false;
            let mode = if executable { "x" } else { "-" };
            lines.insert(
                key.clone(),
                format!("{key}\t{mode}\t{}", crate::staging::sha256(&path)?),
            );
        }
    }
    Ok(())
}

pub(super) fn inputs(context: &Path, exclude: bool) -> io::Result<Vec<String>> {
    let mut lines = BTreeMap::new();
    record(context, context, exclude, &mut lines)?;
    Ok(lines.into_values().collect())
}
pub(super) fn hash(inputs: &[String], args: &BTreeMap<String, String>, parent: &str) -> String {
    let mut hash = Sha256::new();
    hash.update("platform-images/v1");
    for input in inputs {
        hash.update(b"\n");
        hash.update(input);
    }
    for (key, value) in args {
        hash.update(format!("\narg\t{key}={value}"));
    }
    hash.update(format!("\nparent\t{parent}"));
    hex::encode(hash.finalize())[..20].to_owned()
}
