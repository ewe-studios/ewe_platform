//! Check how first-party packages declare and use their error dependencies.
use crate::CiEnvironment;
use clap::Args;
use foundation_tasks::{TaskContext, TaskError, TaskResult, TaskSet};
use std::{
    collections::BTreeSet,
    fs,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
};

#[derive(Args)]
struct Check {
    #[arg(long)]
    root: Option<PathBuf>,
}

pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add(
        "check-errstacks",
        "Check error trace use in first-party packages",
        |ctx, args| blocking::unblock(move || check(&ctx, args)),
    )?;
    set.expose("check-errstacks", &["check-errstacks"])
}
fn scopes(doc: &toml::Table, name: &str) -> BTreeSet<&'static str> {
    let targets = doc
        .get("target")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(toml::Table::values)
        .filter_map(toml::Value::as_table);
    let mut used = BTreeSet::new();
    for group in std::iter::once(doc).chain(targets) {
        for kind in ["dependencies", "build-dependencies", "dev-dependencies"] {
            if group
                .get(kind)
                .and_then(toml::Value::as_table)
                .is_some_and(|deps| {
                    deps.iter().any(|(alias, options)| {
                        options
                            .get("package")
                            .and_then(toml::Value::as_str)
                            .unwrap_or(alias)
                            == name
                    })
                })
            {
                used.insert(kind);
            }
        }
    }
    used
}
fn uses(dir: &Path) -> io::Result<bool> {
    if !dir.is_dir() {
        return Ok(false);
    }
    let mut files = Vec::new();
    super::guards::collect(dir, &mut files)?;
    for path in files {
        for line in BufReader::new(fs::File::open(path)?).split(b'\n') {
            if line?
                .windows(b"ErrorTrace".len())
                .any(|w| w == b"ErrorTrace")
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
fn findings(root: &Path, member: &Path, found: &mut Vec<String>) -> TaskResult {
    if member
        .file_name()
        .is_some_and(|name| name == "foundation_boringtun")
    {
        return Ok(());
    }
    let path = member.join("Cargo.toml");
    if !path.is_file() {
        return Ok(());
    }
    let rel = path
        .strip_prefix(root)
        .map_err(TaskError::operation)?
        .display()
        .to_string()
        .replace('\\', "/");
    let doc: toml::Table = toml::from_str(&fs::read_to_string(path).map_err(TaskError::operation)?)
        .map_err(TaskError::operation)?;
    if !scopes(&doc, "thiserror").is_empty() {
        found.push(format!(
            "{rel}: declares thiserror (use foundation_errstacks)"
        ));
    }
    let used = scopes(&doc, "foundation_errstacks");
    if used.is_empty() {
        return Ok(());
    }
    let dirs: &[&str] = if used == BTreeSet::from(["dev-dependencies"]) {
        &["src", "tests", "examples", "benches"]
    } else {
        &["src"]
    };
    for dir in dirs {
        if uses(&member.join(dir)).map_err(TaskError::operation)? {
            return Ok(());
        }
    }
    let names = dirs
        .iter()
        .map(|d| format!("{d}/"))
        .collect::<Vec<_>>()
        .join(", ");
    found.push(format!(
        "{rel}: declares foundation_errstacks but {names} never uses ErrorTrace"
    ));
    Ok(())
}
fn check(ctx: &TaskContext, args: Check) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let root = args
        .root
        .map_or_else(|| host.root.clone(), |p| host.root.join(p));
    let mut found = Vec::new();
    for dir in ["backends", "crates"] {
        let entries = match fs::read_dir(root.join(dir)) {
            Ok(v) => v,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(TaskError::operation(e)),
        };
        for item in entries {
            if ctx.cancellation().is_cancelled() {
                return Err(TaskError::Cancelled);
            }
            findings(
                &root,
                &item.map_err(TaskError::operation)?.path(),
                &mut found,
            )?;
        }
    }
    if found.is_empty() {
        println!("errstacks guard: ok (every first-party crate uses foundation_errstacks)");
        Ok(())
    } else {
        found.sort();
        Err(TaskError::operation(found.join("\n")))
    }
}
