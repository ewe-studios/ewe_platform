//! Build and type-check the foreign language bindings from the fixture crate.
use super::versions;
use crate::CiEnvironment;
use clap::{Args, ValueEnum};
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use regex::Regex;
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, ValueEnum, PartialEq, Eq)]
enum Scope {
    Full,
    Apple,
}
#[derive(Args)]
pub(super) struct Proof {
    #[arg(long,value_enum,default_value_t=Scope::Full)]
    scope: Scope,
}

struct Host<'a> {
    ctx: &'a TaskContext,
    dir: &'a Path,
    developer: Option<&'a OsStr>,
    rust: &'a str,
}
impl Host<'_> {
    fn command(&self, program: impl AsRef<OsStr>) -> ProcessCommand {
        let mut cmd = ProcessCommand::new(program.as_ref())
            .current_dir(self.dir)
            .env("RUSTUP_TOOLCHAIN", self.rust);
        if let Some(developer) = self.developer {
            cmd = cmd.env("DEVELOPER_DIR", developer);
        }
        cmd
    }
    async fn value(&self, name: &str, args: &[&str], pattern: &str) -> Result<String, TaskError> {
        versions::read(
            self.ctx,
            self.command(name).args(args.iter().copied()),
            pattern,
        )
        .await?
        .map_err(|why| TaskError::operation(format!("{name}: {why}")))
    }
    async fn generate(&self, language: &str) -> TaskResult {
        self.ctx
            .exec(
                self.command(self.dir.join("target/debug/uniffi-bindgen"))
                    .args([
                        "generate",
                        "--library",
                        "target/debug/libuniffi_proof.dylib",
                        "--language",
                        language,
                        "--out-dir",
                        language,
                        "--no-format",
                    ]),
            )
            .await
    }
}

fn artifact(path: &Path) -> TaskResult {
    if fs::metadata(path).map_err(TaskError::operation)?.len() == 0 {
        return Err(TaskError::operation(format!(
            "Empty output: {}",
            path.display()
        )));
    }
    Ok(())
}

async fn swift(host: &Host<'_>, sdk: &str) -> TaskResult {
    host.generate("swift").await?;
    let swift = "swift/uniffi_proof.swift";
    let source = fs::read_to_string(host.dir.join(swift)).map_err(TaskError::operation)?;
    if !Regex::new("func pushOps.*async")
        .map_err(TaskError::operation)?
        .is_match(&source)
    {
        return Err(TaskError::operation(
            "The Swift binding has no async pushOps function",
        ));
    }
    let sdk_path = host
        .value(
            "xcrun",
            &["--sdk", "iphonesimulator", "--show-sdk-path"],
            "^(.+)$",
        )
        .await?;
    let arch = host.value("uname", &["-m"], "^(.+)$").await?;
    let target = format!("{arch}-apple-ios{sdk}-simulator");
    let module = format!(
        "-fmodule-map-file={}",
        host.dir.join("swift/uniffi_proofFFI.modulemap").display()
    );
    let compiler = host.command("xcrun").args([
        "--sdk",
        "iphonesimulator",
        "swiftc",
        "-sdk",
        &sdk_path,
        "-target",
        &target,
    ]);
    host.ctx
        .exec(
            compiler
                .clone()
                .args(["-typecheck", "-Xcc", &module, swift]),
        )
        .await?;
    host.ctx
        .exec(compiler.args(["-emit-object", "-Xcc", &module, swift, "-o", "proof.o"]))
        .await?;
    artifact(&host.dir.join("proof.o"))
}

fn jar(root: &Path, folder: &str, prefix: &str) -> Result<PathBuf, TaskError> {
    let mut matches = Vec::new();
    for dir in fs::read_dir(root).map_err(TaskError::operation)? {
        let dir = dir.map_err(TaskError::operation)?;
        if !dir.file_name().to_string_lossy().starts_with("gradle-") {
            continue;
        }
        let path = dir.path().join(folder);
        if !path.is_dir() {
            continue;
        }
        for item in fs::read_dir(path).map_err(TaskError::operation)? {
            let item = item.map_err(TaskError::operation)?;
            let name = item.file_name();
            let name = name.to_string_lossy();
            if name.strip_prefix(prefix).is_some_and(|rest| {
                rest.starts_with(|c: char| c.is_ascii_digit())
                    && Path::new(rest).extension().is_some_and(|ext| ext == "jar")
            }) && item.file_type().map_err(TaskError::operation)?.is_file()
            {
                matches.push(item.path());
            }
        }
    }
    if matches.len() != 1 {
        return Err(TaskError::operation(format!(
            "Expected one {prefix} jar in {}",
            root.display()
        )));
    }
    Ok(matches.remove(0))
}

async fn kotlin(host: &Host<'_>, root: &Path) -> Result<String, TaskError> {
    let cmd = host
        .command("mise")
        .current_dir(root)
        .args(["where", "gradle"]);
    let gradle = versions::read(host.ctx, cmd, "^(.+)$")
        .await?
        .map_err(TaskError::operation)?;
    let gradle = Path::new(&gradle);
    let classpath = std::env::join_paths([
        jar(gradle, "lib/plugins", "jna-")?,
        jar(gradle, "lib", "kotlinx-coroutines-core-jvm-")?,
    ])
    .map_err(TaskError::operation)?;
    host.generate("kotlin").await?;
    let binding = "kotlin/uniffi/uniffi_proof/uniffi_proof.kt";
    let code = fs::read_to_string(host.dir.join(binding)).map_err(TaskError::operation)?;
    if !Regex::new(r"suspend fun (pushOps|`pushOps`)\(")
        .map_err(TaskError::operation)?
        .is_match(&code)
    {
        return Err(TaskError::operation(
            "The Kotlin binding has no suspend pushOps function",
        ));
    }
    host.ctx
        .exec(
            host.command("kotlinc")
                .arg("-classpath")
                .arg(classpath)
                .args([binding, "-d", "proof.jar"]),
        )
        .await?;
    artifact(&host.dir.join("proof.jar"))?;
    host.value("kotlinc", &["-version"], "^(.*kotlinc-jvm.*)$")
        .await
}

fn record(path: &Path, report: &[String]) -> TaskResult {
    let start = "<!-- uniffi-proof:start -->";
    let end = "<!-- uniffi-proof:end -->";
    let list = report
        .iter()
        .map(|line| format!("- {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let section = format!("{start}\n## UniFFI toolchain proof\n\n{list}\n{end}");
    let old = fs::read_to_string(path).map_err(TaskError::operation)?;
    let new = if let Some((prefix, tail)) = old.split_once(start) {
        let (_, suffix) = tail
            .split_once(end)
            .ok_or_else(|| TaskError::operation("The UniFFI proof section has no end marker"))?;
        format!("{prefix}{section}{suffix}")
    } else {
        format!("{}\n\n{section}\n", old.trim_end())
    };
    if new != old {
        fs::write(path, new).map_err(TaskError::operation)?;
    }
    Ok(())
}

pub(super) async fn run(ctx: TaskContext, args: Proof) -> TaskResult {
    let env = ctx.require::<CiEnvironment>()?;
    let root = &env.root;
    let os = versions::value(&ctx, "uname", &["-s"], "^(.+)$", None)
        .await?
        .map_err(TaskError::operation)?;
    if os != "Darwin" {
        return Err(TaskError::operation(
            "The UniFFI proof requires macOS and the Xcode iOS simulator SDK",
        ));
    }
    let mut developer = env.get("DEVELOPER_DIR");
    let xcode = Path::new("/Applications/Xcode.app/Contents/Developer");
    if developer.is_none()
        && xcode.is_dir()
        && versions::value(&ctx, "xcode-select", &["-p"], "^(.+)$", None)
            .await?
            .as_deref()
            == Ok("/Library/Developer/CommandLineTools")
    {
        developer = Some(xcode.as_os_str().to_owned());
    }
    let rust = versions::value(
        &ctx,
        "rustup",
        &["show", "active-toolchain"],
        r"^(\S+)",
        None,
    )
    .await?
    .map_err(TaskError::operation)?;
    let work = tempfile::tempdir().map_err(TaskError::operation)?;
    let dir = work.path();
    fs::create_dir(dir.join("src")).map_err(TaskError::operation)?;
    let fixture = root.join("scripts/toolchain/proof");
    for name in ["Cargo.lock", "Cargo.toml", "src/lib.rs", "src/bindgen.rs"] {
        fs::copy(fixture.join(name), dir.join(name)).map_err(TaskError::operation)?;
    }
    let host = Host {
        ctx: &ctx,
        dir,
        developer: developer.as_deref(),
        rust: &rust,
    };
    ctx.exec(
        host.command("cargo")
            .env_remove("CARGO_PROFILE_DEV_CODEGEN_BACKEND")
            .args(["build", "--locked", "--target-dir", "target"]),
    )
    .await?;
    let sdk = host
        .value(
            "xcrun",
            &["--sdk", "iphonesimulator", "--show-sdk-version"],
            "^(.+)$",
        )
        .await?;
    swift(&host, &sdk).await?;
    let manifest: toml::Value =
        toml::from_str(&fs::read_to_string(dir.join("Cargo.toml")).map_err(TaskError::operation)?)
            .map_err(TaskError::operation)?;
    let uniffi = manifest
        .get("dependencies")
        .and_then(|t| t.get("uniffi"))
        .and_then(|t| t.get("version"))
        .and_then(toml::Value::as_str)
        .and_then(|v| v.strip_prefix('='))
        .ok_or_else(|| TaskError::operation("Missing pinned UniFFI version"))?;
    let mut report = vec![
        format!(
            "Rust: {}",
            host.value("rustc", &["--version"], "^(.+)$").await?
        ),
        format!("UniFFI: {uniffi}"),
        format!(
            "Swift: {}",
            host.value("xcrun", &["swiftc", "--version"], "^(.+)$")
                .await?
        ),
        format!(
            "Xcode: {}",
            host.value("xcodebuild", &["-version"], "^(.+)$").await?
        ),
        format!("iOS simulator SDK: {sdk}"),
    ];
    if args.scope == Scope::Full {
        report.push(format!("Kotlin: {}", kotlin(&host, root).await?));
        record(
            &root.join("specifications/69-foundation-ui-v2/learnings.md"),
            &report,
        )?;
    }
    println!("{}", report.join("\n"));
    work.close().map_err(TaskError::operation)
}
