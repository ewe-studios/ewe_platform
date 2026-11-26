//! List packages, explain changes, and write matrices for workflow jobs.
use crate::{CiEnvironment, Workspace};
use clap::Args;
use foundation_tasks::{TaskContext, TaskError, TaskResult, TaskSet};
use serde::Serialize;
use serde_json::json;
use std::{cmp::Reverse, collections::BTreeSet, fmt::Write as _, fs::OpenOptions, io::Write as _};

#[derive(Args)]
struct List {}
#[derive(Args)]
struct Plan {
    #[arg(long)]
    packages: Option<String>,
    #[arg(long)]
    base: Option<String>,
    #[arg(long)]
    head: Option<String>,
}
#[derive(Args)]
struct Affected {
    #[arg(long)]
    base: Option<String>,
    #[arg(long)]
    head: Option<String>,
    #[arg(long)]
    json: bool,
}
pub(crate) fn register(set: &mut TaskSet) -> TaskResult {
    set.add("list", "List packages and their check settings", list)?;
    set.add("plan", "Build the workflow matrices", plan)?;
    set.add("affected", "Show which packages a change affects", affected)?;
    for name in ["list", "plan", "affected"] {
        set.expose(name, &[name])?;
    }
    Ok(())
}
async fn list(ctx: TaskContext, _: List) -> TaskResult {
    let ws = Workspace::load(&ctx).await?;
    for (name, p) in ws.packages {
        let s = &ws.settings[&name];
        let status = s.skip.as_ref().map_or_else(
            || {
                format!(
                    "fmt={} build={} clippy={}{} test={} wasm={}",
                    s.fmt,
                    s.build,
                    s.clippy,
                    if s.clippy_deny_warnings { "+deny" } else { "" },
                    s.test,
                    s.wasm
                )
            },
            |reason| format!("SKIP: {reason}"),
        );
        let runs = s
            .runs
            .iter()
            .map(|r| format!("'{r}'"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut env = String::new();
        for (key, value) in &s.env {
            write!(env, "  {key}={value}").map_err(TaskError::operation)?;
        }
        println!(
            "{name:36} {status:42} runs=[{runs}]{}{env}",
            if p.needs_llama { "  +llama.cpp" } else { "" }
        );
    }
    Ok(())
}
async fn affected(ctx: TaskContext, args: Affected) -> TaskResult {
    let ws = Workspace::load(&ctx).await?;
    let head = args.head.as_deref().unwrap_or("HEAD");
    let (base, reasons) = ws.affected(&ctx, args.base.as_deref(), Some(head)).await?;
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"base":base,"head":head,"affected":reasons}))
                .map_err(TaskError::operation)?
        );
    } else {
        println!(
            "{} of {} package(s) affected by {base}..{head}",
            reasons.len(),
            ws.packages.len()
        );
        for (name, why) in reasons {
            println!("{name:36} {why}");
        }
    }
    Ok(())
}
#[derive(Serialize)]
struct Linux<'a> {
    package: &'a str,
    llama: bool,
    timeout: usize,
    #[serde(rename = "container-options")]
    container_options: &'a str,
}
#[derive(Serialize)]
struct Shard {
    #[serde(rename = "shard")]
    label: String,
    index: usize,
    packages: String,
    weight: usize,
    timeout: usize,
    llama: bool,
}
fn shards(
    ws: &Workspace,
    os: &str,
    entries: &[Linux<'_>],
) -> Result<(usize, Vec<Shard>), TaskError> {
    let mut items: Vec<_> = entries
        .iter()
        .filter_map(|e| {
            let s = &ws.settings[e.package];
            let runs = s.platform_runs(os)?;
            (s.test != "off").then_some((e, runs.len()))
        })
        .collect();
    items.sort_by_key(|(e, w)| (Reverse(*w), e.package));
    let count = items.len();
    let total = ws.policy.positive(&format!("{os}-shards"), 4)?.min(count);
    let timeout = ws.policy.positive("shard-timeout-minutes", 300)?;
    let mut groups: Vec<(usize, Vec<&Linux>)> = (0..total).map(|_| (0, Vec::new())).collect();
    for (e, weight) in items {
        let group = groups
            .iter_mut()
            .min_by_key(|g| g.0)
            .ok_or_else(|| TaskError::operation("missing shard"))?;
        group.0 += weight;
        group.1.push(e);
    }
    Ok((
        count,
        groups
            .into_iter()
            .enumerate()
            .map(|(i, (weight, mut group))| {
                group.sort_by_key(|e| e.package);
                Shard {
                    label: format!("{}/{total}", i + 1),
                    index: i + 1,
                    packages: group
                        .iter()
                        .map(|e| e.package)
                        .collect::<Vec<_>>()
                        .join(" "),
                    weight,
                    timeout: group.iter().map(|e| e.timeout).sum::<usize>().min(timeout),
                    llama: group.iter().any(|e| e.llama),
                }
            })
            .collect(),
    ))
}
fn append(env: &CiEnvironment, key: &str, text: &str) -> TaskResult {
    if let Some(path) = env.get(key).filter(|p| !p.is_empty()) {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| file.write_all(text.as_bytes()))
            .map_err(TaskError::operation)?;
    }
    Ok(())
}
async fn selection(
    ws: &Workspace,
    ctx: &TaskContext,
    env: &CiEnvironment,
    args: Plan,
) -> Result<(BTreeSet<String>, String), TaskError> {
    let requested = args
        .packages
        .or_else(|| env.text("CI_PACKAGES"))
        .unwrap_or_default();
    let base = args.base.or_else(|| env.text("CI_BASE_SHA"));
    let head = args.head.or_else(|| env.text("CI_HEAD_SHA"));
    let all = || ws.packages.keys().cloned().collect::<BTreeSet<_>>();
    let (names, why) = match requested.trim() {
        "all" => (all(), "all packages requested".into()),
        "" => {
            if let Some(base) = base.as_deref().filter(|x| x.bytes().any(|c| c != b'0')) {
                match ws.affected(ctx, Some(base), head.as_deref()).await {
                    Ok((_, reasons)) => {
                        let n = reasons.len();
                        (
                            reasons.into_keys().collect(),
                            format!("{n} package(s) affected by the change"),
                        )
                    }
                    Err(e @ TaskError::Exit { .. }) => {
                        eprintln!("cannot diff against {base} ({e}); full run");
                        (all(), "full run (base commit not available)".into())
                    }
                    Err(e) => return Err(e),
                }
            } else {
                (all(), "full run".into())
            }
        }
        requested => {
            let names: BTreeSet<_> = requested
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            for name in &names {
                if !ws.packages.contains_key(name) {
                    return Err(TaskError::operation(format!("unknown package: {name}")));
                }
            }
            (names, "requested explicitly".into())
        }
    };
    Ok((names, why))
}
async fn plan(ctx: TaskContext, args: Plan) -> TaskResult {
    let ws = Workspace::load(&ctx).await?;
    let env = ctx.require::<CiEnvironment>()?;
    let (names, why) = selection(&ws, &ctx, &env, args).await?;
    let entries: Vec<_> = names
        .iter()
        .filter_map(|name| {
            let s = &ws.settings[name];
            s.skip.is_none().then_some(Linux {
                package: name,
                llama: ws.packages[name].needs_llama,
                timeout: s.timeout_minutes,
                container_options: &s.container_options,
            })
        })
        .collect();
    let matrix = json!({"include":entries});
    let count = entries.len();
    let summary = format!("{count} package(s) selected ({why})");
    eprintln!("{summary}");
    let mut output = format!("matrix={matrix}\ncount={count}\n");
    let mut report = format!("### CI plan\n\n{summary}\n\n");
    for item in &entries {
        writeln!(
            report,
            "- `{}`{}",
            item.package,
            if item.llama { " (needs llama.cpp)" } else { "" }
        )
        .map_err(TaskError::operation)?;
    }
    for (os, label) in [("macos", "macOS"), ("windows", "Windows")] {
        let (n, groups) = shards(&ws, os, &entries)?;
        writeln!(
            output,
            "{os}-matrix={}\n{os}-count={n}\n{os}-shard-count={}",
            json!({"include":groups}),
            groups.len()
        )
        .map_err(TaskError::operation)?;
        for g in groups {
            writeln!(
                report,
                "\n{label} {} (weight {}): {}",
                g.label, g.weight, g.packages
            )
            .map_err(TaskError::operation)?;
        }
    }
    for (os, default) in [("linux", 8), ("macos", 2), ("windows", 2)] {
        let key = format!("max-parallel-{os}");
        writeln!(output, "{key}={}", ws.policy.positive(&key, default)?)
            .map_err(TaskError::operation)?;
    }
    for name in names {
        if let Some(skip) = &ws.settings[&name].skip {
            writeln!(report, "- ~~`{name}`~~ skipped: {skip}").map_err(TaskError::operation)?;
        }
    }
    append(&env, "GITHUB_OUTPUT", &output)?;
    append(&env, "GITHUB_STEP_SUMMARY", &report)?;
    println!("{matrix}");
    Ok(())
}
