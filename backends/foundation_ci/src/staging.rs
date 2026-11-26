//! Compare generated file trees and replace the selected outputs together.
use foundation_tasks::{TaskError, TaskResult};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
};

pub(crate) fn sha256(path: &Path) -> io::Result<String> {
    let mut reader = BufReader::new(fs::File::open(path)?);
    let mut h = Sha256::new();
    loop {
        let block = reader.fill_buf()?;
        if block.is_empty() {
            break;
        }
        let size = block.len();
        h.update(block);
        reader.consume(size);
    }
    Ok(hex::encode(h.finalize()))
}
fn equal_files(left: &Path, right: &Path) -> io::Result<bool> {
    let a = fs::File::open(left)?;
    let b = fs::File::open(right)?;
    if a.metadata()?.len() != b.metadata()?.len() {
        return Ok(false);
    }
    let (mut a, mut b) = (BufReader::new(a), BufReader::new(b));
    loop {
        let (left, right) = (a.fill_buf()?, b.fill_buf()?);
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

fn equal(left: &Path, right: &Path) -> io::Result<bool> {
    let left_type = fs::symlink_metadata(left)?.file_type();
    let right_type = match fs::symlink_metadata(right) {
        Ok(value) => value.file_type(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    if left_type.is_symlink() && right_type.is_symlink() {
        return Ok(left.read_link()? == right.read_link()?);
    }
    if left_type.is_file() && right_type.is_file() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if left.metadata()?.permissions().mode() != right.metadata()?.permissions().mode() {
                return Ok(false);
            }
        }
        return equal_files(left, right);
    }
    if !left_type.is_dir() || !right_type.is_dir() {
        return Ok(false);
    }
    let names = |parent: &Path| -> io::Result<Vec<_>> {
        let mut items = fs::read_dir(parent)?
            .map(|d| d.map(|v| v.file_name()))
            .collect::<io::Result<Vec<_>>>()?;
        items.sort();
        Ok(items)
    };
    let left_names = names(left)?;
    if left_names != names(right)? {
        return Ok(false);
    }
    for name in left_names {
        if !equal(&left.join(&name), &right.join(name))? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) fn lock(root: &Path) -> io::Result<fs::File> {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut id = DefaultHasher::new();
    root.canonicalize()?.hash(&mut id);
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(std::env::temp_dir().join(format!("ewe-generated-{:x}.lock", id.finish())))?;
    file.lock()?;
    Ok(file)
}

struct Move {
    source: PathBuf,
    target: PathBuf,
    backup: Option<PathBuf>,
    installed: bool,
}
fn restore(moves: &[Move]) -> io::Result<()> {
    for item in moves.iter().rev() {
        if item.installed {
            fs::rename(&item.target, &item.source)?;
        }
        if let Some(old) = &item.backup {
            fs::rename(old, &item.target)?;
        }
    }
    Ok(())
}

pub(crate) fn publish<P: AsRef<Path>>(
    stage: tempfile::TempDir,
    root: &Path,
    pairs: impl IntoIterator<Item = (P, PathBuf)>,
    check: bool,
) -> TaskResult {
    let _lock = lock(root).map_err(TaskError::operation)?;
    let mut moves = Vec::new();
    for (name, target) in pairs {
        let source = stage.path().join(name);
        if !equal(&source, &target).map_err(TaskError::operation)? {
            if check {
                return Err(TaskError::operation(format!(
                    "Generated output differs: {}",
                    target.display()
                )));
            }
            moves.push(Move {
                source,
                target,
                backup: None,
                installed: false,
            });
        }
    }
    let update = (|| {
        for (index, item) in moves.iter_mut().enumerate() {
            fs::create_dir_all(
                item.target
                    .parent()
                    .ok_or_else(|| io::Error::other("Missing output parent"))?,
            )?;
            let backup = stage.path().join(format!("previous-{index}"));
            match fs::rename(&item.target, &backup) {
                Ok(()) => item.backup = Some(backup),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
            fs::rename(&item.source, &item.target)?;
            item.installed = true;
        }
        Ok::<_, io::Error>(())
    })();
    if let Err(error) = update {
        if let Err(rollback) = restore(&moves) {
            return Err(TaskError::operation(format!(
                "{error}; rollback failed: {rollback}; previous files: {}",
                stage.keep().display()
            )));
        }
        return Err(TaskError::operation(error));
    }
    stage.close().map_err(TaskError::operation)
}
