//! Compare directory contents and replace generated files through a staging directory.
use foundation_tasks::{Cancellation, TaskError, TaskResult};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

#[derive(PartialEq, Eq)]
enum Entry {
    Directory,
    File(String),
    Link(PathBuf),
}
fn cancelled(cancel: &Cancellation) -> TaskResult {
    if cancel.is_cancelled() {
        Err(TaskError::Cancelled)
    } else {
        Ok(())
    }
}
fn scan(
    root: &Path,
    relative: &Path,
    contents: &mut BTreeMap<PathBuf, Entry>,
    cancel: &Cancellation,
) -> TaskResult {
    for child in fs::read_dir(root.join(relative)).map_err(TaskError::operation)? {
        cancelled(cancel)?;
        let child = child.map_err(TaskError::operation)?;
        let path = relative.join(child.file_name());
        let ty = child.file_type().map_err(TaskError::operation)?;
        let entry = if ty.is_symlink() {
            Entry::Link(fs::read_link(child.path()).map_err(TaskError::operation)?)
        } else if ty.is_dir() {
            scan(root, &path, contents, cancel)?;
            Entry::Directory
        } else if ty.is_file() {
            Entry::File(super::stage::digest_file(&child.path())?)
        } else {
            return Err(TaskError::operation(format!(
                "Unsupported file {}",
                path.display()
            )));
        };
        contents.insert(path, entry);
    }
    Ok(())
}
pub(super) fn replace(stage: &Path, dest: &Path, locks: &Path) -> io::Result<()> {
    use sha2::{Digest, Sha256};
    let parent = dest
        .parent()
        .ok_or_else(|| io::Error::other("Missing generated directory"))?;
    fs::create_dir_all(parent)?;
    let name = dest
        .file_name()
        .ok_or_else(|| io::Error::other("Missing directory name"))?;
    let key = hex::encode(Sha256::digest(
        parent
            .canonicalize()?
            .join(name)
            .as_os_str()
            .as_encoded_bytes(),
    ));
    let locks = locks.join("ewe-native-staging");
    fs::create_dir_all(&locks)?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(locks.join(key))?;
    lock.lock()?;
    let scratch = tempfile::tempdir_in(parent)?;
    let old = scratch.path().join("previous");
    let previous = match fs::rename(dest, &old) {
        Ok(()) => true,
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(e) => return Err(e),
    };
    if let Err(e) = fs::rename(stage, dest) {
        if previous {
            if let Err(restore) = fs::rename(&old, dest) {
                let recovery = scratch.keep();
                return Err(io::Error::other(format!(
                    "{e}. Restoring the previous files failed: {restore}. Recover files from {}",
                    recovery.display()
                )));
            }
        }
        return Err(e);
    }
    scratch.close()
}
pub(super) fn sync(
    from: &Path,
    into: &Path,
    check: bool,
    locks: &Path,
    cancel: &Cancellation,
) -> TaskResult {
    let mut source = BTreeMap::new();
    scan(from, Path::new(""), &mut source, cancel)?;
    if check {
        let mut dest = BTreeMap::new();
        scan(into, Path::new(""), &mut dest, cancel)?;
        if let Some(path) = source
            .keys()
            .chain(dest.keys())
            .find(|path| source.get(*path) != dest.get(*path))
        {
            return Err(TaskError::operation(format!(
                "Generated file differs: {}",
                into.join(path).display()
            )));
        }
        return Ok(());
    }
    let parent = into
        .parent()
        .ok_or_else(|| TaskError::operation("Missing destination parent"))?;
    fs::create_dir_all(parent).map_err(TaskError::operation)?;
    let scratch = tempfile::tempdir_in(parent).map_err(TaskError::operation)?;
    let stage = scratch.path().join("content");
    fs::create_dir(&stage).map_err(TaskError::operation)?;
    for (path, entry) in source {
        cancelled(cancel)?;
        let output = stage.join(&path);
        match entry {
            Entry::Directory => fs::create_dir(output),
            Entry::File(_) => fs::copy(from.join(path), output).map(drop),
            Entry::Link(target) => {
                #[cfg(unix)]
                {
                    std::os::unix::fs::symlink(target, output)
                }
                #[cfg(windows)]
                {
                    if from.join(path).is_dir() {
                        std::os::windows::fs::symlink_dir(target, output)
                    } else {
                        std::os::windows::fs::symlink_file(target, output)
                    }
                }
            }
        }
        .map_err(TaskError::operation)?;
    }
    cancelled(cancel)?;
    replace(&stage, into, locks).map_err(TaskError::operation)?;
    scratch.close().map_err(TaskError::operation)
}
