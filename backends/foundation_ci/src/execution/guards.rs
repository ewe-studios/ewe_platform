//! Inspect first-party Rust sources for violations of runtime ownership.
use crate::CiEnvironment;
use clap::Args;
use foundation_tasks::{TaskContext, TaskError, TaskResult, TaskSet};
use regex::Regex;
use std::{
    collections::BTreeSet,
    io::{self, BufRead},
    path::{Path, PathBuf},
};

#[derive(Args)]
struct ArgsForGuard {
    #[arg(long)]
    root: Option<PathBuf>,
    #[arg(long)]
    allowlist: Option<PathBuf>,
    #[arg(long)]
    check: bool,
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum Guard {
    Wake,
    Time,
    Clock,
}
impl Guard {
    fn rule(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Guard::Wake => (
                "executor wake",
                "executor_wake_allowlist.txt",
                include_str!("executor-wake.regex"),
            ),
            Guard::Time => (
                "time source",
                "time_sources_allowlist.txt",
                include_str!("time-sources.regex"),
            ),
            Guard::Clock => (
                "clock parameter",
                "clock_params_allowlist.txt",
                include_str!("clock-params.regex"),
            ),
        }
    }
}
pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    for (name, kind) in [
        ("check-executor-wake", Guard::Wake),
        ("check-time-sources", Guard::Time),
        ("check-clock-params", Guard::Clock),
    ] {
        set.add(
            name,
            "Check runtime ownership in Rust sources",
            move |ctx, args| blocking::unblock(move || scan(&ctx, args, kind)),
        )?;
        set.expose(name, &[name])?;
    }
    Ok(())
}
pub(super) fn collect(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let path = entry.path();
        if ty.is_dir() {
            if !matches!(
                entry.file_name().to_str(),
                Some("target" | ".git" | "node_modules" | ".claude" | ".worktrees")
            ) {
                collect(&path, files)?;
            }
        } else if ty.is_file() && path.extension().is_some_and(|s| s == "rs") {
            files.push(path);
        }
    }
    Ok(())
}
fn source_files(root: &Path, kind: Guard) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if kind == Guard::Wake {
        collect(root, &mut files)?;
    } else {
        for directory in ["backends", "crates", "bin", "infrastructure", "apps"] {
            if directory == "apps" && kind != Guard::Clock {
                continue;
            }
            let crates = match std::fs::read_dir(root.join(directory)) {
                Ok(entries) => entries,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            for entry in crates {
                let source = entry?.path().join("src");
                if source.is_dir() {
                    collect(&source, &mut files)?;
                }
            }
        }
    }
    files.sort();
    Ok(files)
}
fn scanned(rel: &str, kind: Guard) -> bool {
    if kind == Guard::Wake {
        return !rel.starts_with("backends/foundation_core/src/valtron/");
    }
    let parts: Vec<_> = rel.split('/').take(3).collect();
    if parts.len() != 3 || parts[2] != "src" {
        return false;
    }
    if !(matches!(parts[0], "backends" | "crates" | "bin" | "infrastructure")
        || kind == Guard::Clock && parts[0] == "apps")
    {
        return false;
    }
    if kind == Guard::Time {
        return parts[1] != "foundation_boringtun";
    }
    ![
        "backends/foundation_context/",
        "backends/foundation_compact/",
        "backends/foundation_nostd/",
        "backends/foundation_core/src/valtron/",
    ]
    .iter()
    .any(|p| rel.starts_with(p))
}
fn read_allowlist(path: &Path) -> Result<String, TaskError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(TaskError::operation(error)),
    }
}
fn scan(ctx: &TaskContext, args: ArgsForGuard, kind: Guard) -> TaskResult {
    let host = ctx.require::<CiEnvironment>()?;
    let repo = host
        .root
        .ancestors()
        .find(|p| p.join("ci/packages.toml").is_file())
        .unwrap_or(&host.root);
    let root = args.root.unwrap_or_else(|| repo.to_owned());
    let (label, filename, pattern) = kind.rule();
    let path = args
        .allowlist
        .unwrap_or_else(|| repo.join("scripts/ci").join(filename));
    let allowlist = read_allowlist(&path)?;
    let allowed: Vec<_> = allowlist
        .lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter(|s| !s.is_empty())
        .collect();
    let regex = Regex::new(pattern.trim()).map_err(TaskError::operation)?;
    let files = source_files(&root, kind).map_err(TaskError::operation)?;
    let mut inventory = Vec::new();
    let mut marked_count = 0;
    let mut allowed_count = 0;
    let mut violations = Vec::new();
    let mut used = BTreeSet::new();
    for path in files {
        if ctx.cancellation().is_cancelled() {
            return Err(TaskError::Cancelled);
        }
        let rel = path
            .strip_prefix(&root)
            .map_err(TaskError::operation)?
            .to_string_lossy()
            .replace('\\', "/");
        if !scanned(&rel, kind) {
            continue;
        }
        let entry = allowed
            .iter()
            .copied()
            .find(|e| *e == rel || (kind != Guard::Wake && e.ends_with('/') && rel.starts_with(e)));
        let file = std::fs::File::open(path).map_err(TaskError::operation)?;
        let mut count = 0;
        for (i, line) in io::BufReader::new(file).split(b'\n').enumerate() {
            let bytes = line.map_err(TaskError::operation)?;
            let line = String::from_utf8_lossy(&bytes);
            if regex.is_match(&line) {
                count += 1;
                if kind == Guard::Time && line.contains("// time-source:") {
                    marked_count += 1;
                    continue;
                }
                if let Some(e) = entry {
                    used.insert(e);
                    allowed_count += 1;
                } else {
                    violations.push(format!("{rel}:{}: {}", i + 1, line.trim()));
                }
            }
        }
        if count > 0 {
            inventory.push((rel, count));
        }
    }
    if kind != Guard::Wake {
        for entry in allowed {
            if !used.contains(entry) {
                let rule = if kind == Guard::Time {
                    "matches no direct read"
                } else {
                    "matches no hit"
                };
                violations.push(format!("unused allowlist entry: {entry} {rule}"));
            }
        }
    }
    if violations.is_empty() {
        let hits: usize = inventory.iter().map(|(_, count)| *count).sum();
        if !args.check && kind != Guard::Wake {
            for (path, count) in &inventory {
                println!("  {path}: {count}");
            }
        }
        let files = inventory.len();
        let summary = match kind {
            Guard::Wake => "no use outside backends/foundation_core/src/valtron/".to_owned(),
            Guard::Time => format!(
                "{hits} direct reads in {files} files: {allowed_count} allowlisted, {marked_count} marked, 0 violations"
            ),
            Guard::Clock => format!(
                "{hits} clock-parameter hits in {files} files: {allowed_count} allowlisted, 0 violations"
            ),
        };
        println!("{label} guard: ok ({summary})");
        Ok(())
    } else {
        Err(TaskError::operation(format!(
            "{label} guard:\n{}",
            violations.join("\n")
        )))
    }
}
