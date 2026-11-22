//! Generate a task index for a Cargo binary.
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
    fs, io,
    path::{Path, PathBuf},
};

/// Generate the index and mark its inputs as Cargo build dependencies.
///
/// # Errors
/// Reports invalid paths, ambiguous names, or invalid task source files.
pub fn task_dir(binary: &str, source: &str, directory: &str) -> io::Result<()> {
    let package = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR")
            .ok_or_else(|| invalid("Cargo did not supply the package directory"))?,
    )
    .canonicalize()?;
    let output = PathBuf::from(
        std::env::var_os("OUT_DIR")
            .ok_or_else(|| invalid("Cargo did not supply the output directory"))?,
    );
    if binary.is_empty()
        || !binary
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(invalid("Invalid Cargo binary name"));
    }
    let source = package.join(source).canonicalize()?;
    let parent = source
        .parent()
        .filter(|parent| parent.starts_with(&package))
        .ok_or_else(|| invalid("The binary source escapes the package"))?;
    let scan = parent.join(directory);
    let mut entries = BTreeMap::new();
    let mut ancestry = BTreeSet::new();
    visit(&package, &scan, "", &mut entries, &mut ancestry)?;
    let mut declarations = String::new();
    let mut registrations = String::new();
    let mut entries = entries.into_iter().collect::<Vec<_>>();
    entries.sort_by(|(_, (left, _)), (_, (right, _))| left.cmp(right));
    for (index, (name, (file, summary))) in entries.into_iter().enumerate() {
        let path = file
            .to_str()
            .ok_or_else(|| invalid("A Rust task source path must be Unicode"))?;
        writeln!(declarations, "#[path = {path:?}] mod task_{index};").expect("write to String");
        writeln!(
            registrations,
            "set.add({name:?}, {summary:?}, task_{index}::task)?;"
        )
        .expect("write to String");
    }
    let index = output.join(format!("foundation-tasks-{binary}.rs"));
    fs::write(&index, format!("{declarations}\npub(super) fn register(set: &mut TaskSet) -> TaskResult {{\n{registrations}Ok(())\n}}\n"))?;
    let digits = b"0123456789abcdef";
    let key: String = binary
        .bytes()
        .flat_map(|byte| {
            [
                char::from(digits[usize::from(byte >> 4)]),
                char::from(digits[usize::from(byte & 15)]),
            ]
        })
        .collect();
    let variable = format!("FOUNDATION_TASKS_INDEX_{key}");
    println!("cargo:rerun-if-changed={}", source.display());
    println!("cargo:rustc-env={variable}={}", index.display());
    println!("cargo:rustc-env={variable}_DIR={directory}");
    Ok(())
}

fn visit(
    package: &Path,
    directory: &Path,
    prefix: &str,
    entries: &mut BTreeMap<String, (PathBuf, String)>,
    ancestry: &mut BTreeSet<PathBuf>,
) -> io::Result<()> {
    let canonical = confined(package, directory)?;
    if !ancestry.insert(canonical.clone()) {
        return Err(invalid(format!(
            "Task directory contains a cycle: {}",
            directory.display()
        )));
    }
    println!("cargo:rerun-if-changed={}", directory.display());
    let mut items = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    items.sort_by_key(fs::DirEntry::file_name);
    for item in items {
        let path = item.path();
        let canonical = confined(package, &path)?;
        if canonical.is_dir() {
            let leaf = segment(&path, path.file_name())?;
            visit(
                package,
                &path,
                &format!("{prefix}{leaf}:"),
                entries,
                ancestry,
            )?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let leaf = segment(&path, path.file_stem())?;
            let name = format!("{prefix}{leaf}");
            let summary = read_task(&canonical)?;
            if let Some((prior, _)) = entries.get(&name) {
                return Err(invalid(format!(
                    "Task {name} occurs in {} and {}",
                    prior.display(),
                    path.display()
                )));
            }
            println!("cargo:rerun-if-changed={}", path.display());
            if path != canonical {
                println!("cargo:rerun-if-changed={}", canonical.display());
            }
            entries.insert(name, (path, summary));
        }
    }
    ancestry.remove(&canonical);
    Ok(())
}

fn segment(path: &Path, name: Option<&std::ffi::OsStr>) -> io::Result<String> {
    let leaf = name
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("")
        .replace('_', "-");
    super::validate_name(&leaf)
        .map_err(|_| invalid(format!("Invalid task name from {}", path.display())))?;
    Ok(leaf)
}

fn confined(package: &Path, path: &Path) -> io::Result<PathBuf> {
    let canonical = path.canonicalize()?;
    if canonical.starts_with(package) {
        Ok(canonical)
    } else {
        Err(invalid(format!(
            "Task path escapes the package: {}",
            path.display()
        )))
    }
}

fn read_task(path: &Path) -> io::Result<String> {
    let source = fs::read_to_string(path)?;
    let explain = |detail| invalid(format!("{}: {detail}", path.display()));
    let file = syn::parse_file(&source).map_err(|error| explain(error.to_string()))?;
    let mut tasks = file.items.into_iter().filter_map(|item| match item {
        syn::Item::Fn(task) if task.sig.ident == "task" => Some(task),
        _ => None,
    });
    let task = tasks
        .next()
        .ok_or_else(|| explain("Expected a task function".to_owned()))?;
    if tasks.next().is_some()
        || task.sig.asyncness.is_none()
        || task.sig.inputs.len() != 2
        || !matches!(
            task.vis,
            syn::Visibility::Public(_) | syn::Visibility::Restricted(_)
        )
    {
        return Err(explain(
            "Expected one visible async task function with two parameters".to_owned(),
        ));
    }
    let summary = task
        .attrs
        .iter()
        .filter_map(|attr| {
            if let syn::Meta::NameValue(meta) = &attr.meta {
                if meta.path.is_ident("doc") {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(text),
                        ..
                    }) = &meta.value
                    {
                        return Some(text.value());
                    }
                }
            }
            None
        })
        .flat_map(|doc| {
            doc.lines()
                .map(|line| line.trim().to_owned())
                .collect::<Vec<_>>()
        })
        .skip_while(String::is_empty)
        .take_while(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if summary.is_empty() {
        Err(explain("The task function needs a summary".to_owned()))
    } else {
        Ok(summary)
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
