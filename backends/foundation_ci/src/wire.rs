//! Generate and check the workspace wire bindings before dependent packages build.
mod adapters;

use crate::{toolchain, CiEnvironment};
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult, TaskSet};
use std::{
    fmt::Write,
    fs,
    path::{Path, PathBuf},
};

#[derive(Args)]
struct Generate {
    /// Write an independent copy to this directory.
    output: Option<PathBuf>,
}
#[derive(Args)]
struct Check {}
#[derive(Args)]
struct Bundle {
    entry: Option<PathBuf>,
    output: Option<PathBuf>,
}

/// Register the workspace wire generator and its verification commands.
///
/// # Errors
/// Returns a catalog error for duplicate names or paths.
pub fn set() -> Result<TaskSet, TaskError> {
    let mut set = TaskSet::new("foundation_wire_tools").with_namespace("wire");
    set.add(
        "generate",
        "Generate the wire bindings",
        |ctx, args: Generate| generate(ctx, args.output, false),
    )?;
    set.add(
        "check",
        "Compare the generated wire bindings",
        |ctx, _: Check| generate(ctx, None, true),
    )?;
    set.add(
        "bundle",
        "Bundle TypeScript wire bindings",
        |ctx: TaskContext, args: Bundle| async move {
            let host = ctx.require::<CiEnvironment>()?;
            let library = host.root.join("backends/foundation_ui_wire");
            let entry = args
                .entry
                .unwrap_or_else(|| library.join("generated/ts/index.ts"));
            let output = args.output.unwrap_or_else(|| {
                host.root
                    .join("backends/foundation_wasm_ui/runtimes/foundation-ui-wire.js")
            });
            bundle(
                &ctx,
                &library,
                &host.root.join(entry),
                &host.root.join(output),
            )
            .await
        },
    )?;
    for (name, alias) in [
        ("generate", "wire-generate"),
        ("check", "wire-check"),
        ("bundle", "wire-bundle"),
    ] {
        set.expose(name, &[alias])?;
    }
    Ok(set)
}

fn files(parent: &Path) -> Result<Vec<PathBuf>, TaskError> {
    fn collect(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
        for item in fs::read_dir(dir)? {
            let item = item?;
            if item.file_type()?.is_dir() {
                collect(&item.path(), files)?;
            } else {
                files.push(item.path());
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    collect(parent, &mut files).map_err(TaskError::operation)?;
    files.sort();
    Ok(files)
}

async fn bundle(ctx: &TaskContext, library: &Path, entry: &Path, destination: &Path) -> TaskResult {
    let parent = destination
        .parent()
        .ok_or_else(|| TaskError::operation("Missing script destination directory"))?;
    let stage = tempfile::tempdir_in(parent).map_err(TaskError::operation)?;
    let bundled = stage.path().join("foundation-ui-wire.js");
    let mut out = std::ffi::OsString::from("--outfile=");
    out.push(&bundled);
    ctx.exec(
        ProcessCommand::new("pnpm")
            .arg("--dir")
            .arg(library.join("ts"))
            .args(["exec", "esbuild"])
            .arg(entry)
            .args([
                "--bundle",
                "--format=esm",
                "--target=es2022",
                "--minify-whitespace",
                "--log-level=warning",
            ])
            .arg(out)
            .env("NODE_PATH", library.join("ts/node_modules"))
            .env("LC_ALL", "C"),
    )
    .await?;
    fs::rename(bundled, destination).map_err(TaskError::operation)?;
    stage.close().map_err(TaskError::operation)
}

async fn generate(ctx: TaskContext, output: Option<PathBuf>, check: bool) -> TaskResult {
    toolchain::check_wire(ctx.clone()).await?;
    let host = ctx.require::<CiEnvironment>()?;
    let root = &host.root;
    let library = root.join("backends/foundation_ui_wire");
    let asset = output.as_ref().map_or_else(
        || root.join("backends/foundation_wasm_ui/runtimes/foundation-ui-wire.js"),
        |p| root.join(p).join("foundation-ui-wire.js"),
    );
    let output = output.map_or_else(|| library.clone(), |p| root.join(p));
    fs::create_dir_all(&output).map_err(TaskError::operation)?;
    let stage = tempfile::Builder::new()
        .prefix(".ui-wire-generate.")
        .tempdir_in(&output)
        .map_err(TaskError::operation)?;
    let dir = stage.path();
    let schemas = library.join("schema");
    let schema_files = ["common", "state", "ops", "frame", "action", "hello"]
        .map(|name| schemas.join(format!("{name}.fbs")));
    let rust = dir.join("src/generated");
    fs::create_dir_all(&rust).map_err(TaskError::operation)?;
    let input = dir.join("wire.fbs");
    fs::write(&input, "include \"frame.fbs\";\ninclude \"state.fbs\";\n")
        .map_err(TaskError::operation)?;
    let flatc = ProcessCommand::new("flatc").env("LC_ALL", "C");
    ctx.exec(
        flatc
            .clone()
            .args([
                "--rust",
                "--gen-all",
                "--gen-object-api",
                "--rust-module-root-file",
                "--filename-suffix",
                "",
                "-I",
            ])
            .arg(&schemas)
            .arg("-o")
            .arg(&rust)
            .arg(input),
    )
    .await?;
    adapters::rust(&rust)?;
    let config = dir.join("rustfmt.toml");
    fs::write(&config, "").map_err(TaskError::operation)?;
    ctx.exec(
        ProcessCommand::new("rustfmt")
            .args(["--edition", "2021", "--config-path"])
            .arg(config)
            .arg(rust.join("mod.rs")),
    )
    .await?;
    for language in ["ts", "swift", "kotlin"] {
        let dest = dir.join("generated").join(language);
        fs::create_dir_all(&dest).map_err(TaskError::operation)?;
        let mut command = flatc.clone().arg(format!("--{language}"));
        if language == "ts" {
            command = command.arg("--gen-object-api");
        }
        ctx.exec(command.arg("-o").arg(&dest).args(&schema_files))
            .await?;
        match language {
            "ts" => adapters::typescript(&dest)?,
            "swift" => adapters::swift(&dest, &library.join("src/vocab/version.rs"))?,
            _ => adapters::kotlin(&dest)?,
        }
    }
    let entry = typescript_index(&dir.join("generated/ts"), &library)?;
    bundle(&ctx, &library, &entry, &dir.join("foundation-ui-wire.js")).await?;
    let pairs = [
        ("src/generated", output.join("src/generated")),
        ("generated", output.join("generated")),
        ("foundation-ui-wire.js", asset),
    ];
    crate::staging::publish(stage, &output, pairs, check)
}

fn typescript_index(ts: &Path, library: &Path) -> Result<PathBuf, TaskError> {
    let mut index =
        fs::read_to_string(library.join("ts/index.ts")).map_err(TaskError::operation)?;
    let modules = ts.join("ewe/ui");
    for module in files(&modules)? {
        if module.parent() == Some(modules.as_path())
            && module.extension().is_some_and(|ext| ext == "ts")
        {
            let stem = module
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| TaskError::operation("Invalid TypeScript module name"))?;
            writeln!(index, "export * from './ewe/ui/{stem}.js';").map_err(TaskError::operation)?;
        }
    }
    let entry = ts.join("index.ts");
    fs::write(&entry, index).map_err(TaskError::operation)?;
    Ok(entry)
}
