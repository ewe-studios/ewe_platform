//! Build the browser package from the same module graph as the Cargo runtime.
use crate::embedded as js;
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult, TaskSet};
use std::{
    fs, io,
    path::{Path, PathBuf},
    prelude::rust_2021::*,
};

pub(super) const NAME: &str = "@ewe-studios/foundation-wasm-ui";
pub(super) const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Args)]
struct Pack {
    /// Directory that receives the tested npm archive.
    #[arg(long)]
    output: PathBuf,
    /// Reject a release tag that differs from this crate version.
    #[arg(long)]
    tag: Option<String>,
}

fn prepare(root: &Path) -> io::Result<()> {
    let manifest =
        include_str!("../../npm/package.json.template").replace("{{crate_version}}", VERSION);
    let data: serde_json::Value = serde_json::from_str(&manifest)?;
    if data["name"] != NAME || data["version"] != VERSION {
        return Err(io::Error::other(
            "The npm manifest must match the foundation_wasm_ui release version",
        ));
    }
    let ui = root.join("ui/runtimes");
    let core = root.join("foundation_wasm/runtime");
    fs::create_dir_all(&ui)?;
    fs::create_dir_all(&core)?;
    fs::write(
        core.join("foundation-wasm.js"),
        foundation_wasm::embedded::FOUNDATION_WASM_JS,
    )?;
    // The installed command carries its module graph outside the source checkout.
    for (name, contents) in [
        (
            "runtime-entry.js",
            include_str!("../../runtimes/runtime-entry.js"),
        ),
        (
            "foundation-ui-bootstrap.js",
            include_str!("../../runtimes/foundation-ui-bootstrap.js"),
        ),
        ("foundation-wasm-ui.js", js::FOUNDATION_WASM_UI_JS),
        ("foundation-ui-actions.js", js::FOUNDATION_UI_ACTIONS_JS),
        ("foundation-ui-wire.js", js::FOUNDATION_UI_WIRE_JS),
        ("foundation-ui-vocab.js", js::FOUNDATION_UI_VOCAB_JS),
        ("foundation-ui-frames.js", js::FOUNDATION_UI_FRAMES_JS),
        (
            "foundation-ui-connections.js",
            js::FOUNDATION_UI_CONNECTIONS_JS,
        ),
        ("foundation-ui-recovery.js", js::FOUNDATION_UI_RECOVERY_JS),
        ("foundation-ui-live.js", js::FOUNDATION_UI_LIVE_JS),
        (
            "foundation-ui-behaviours.js",
            js::FOUNDATION_UI_BEHAVIOURS_JS,
        ),
        (
            "platform-scheme-interceptor.js",
            js::PLATFORM_SCHEME_INTERCEPTOR_JS,
        ),
    ] {
        fs::write(ui.join(name), contents)?;
    }
    let entry = ui.join("runtime-entry.js").canonicalize()?;
    let bundle = foundation_codegen::javascript::bundle(&entry)
        .map_err(|e| io::Error::other(std::format!("{e:#}")))?;
    for file in fs::read_dir(&ui)? {
        let path = file?.path();
        if !bundle.inputs.contains(&path.canonicalize()?) {
            fs::remove_file(path)?;
        }
    }
    fs::write(root.join("bundle.js"), bundle.readable)?;
    fs::write(root.join("bundle.min.js"), bundle.minified)?;
    fs::write(root.join("package.json"), manifest)?;
    fs::write(root.join("index.js"), include_bytes!("../../npm/index.js"))?;
    fs::write(
        root.join("README.md"),
        include_bytes!("../../npm/README.md"),
    )?;
    fs::write(root.join("LICENSE"), include_bytes!("../../npm/LICENSE"))?;
    Ok(())
}

async fn pack(ctx: TaskContext, args: Pack) -> TaskResult {
    let tag = std::format!("foundation_wasm_ui-v{VERSION}");
    if args.tag.is_some_and(|value| value != tag) {
        return Err(TaskError::operation(std::format!("Expected tag {tag}")));
    }
    fs::create_dir_all(&args.output).map_err(TaskError::operation)?;
    let tmp = tempfile::tempdir_in(&args.output).map_err(TaskError::operation)?;
    let dir = tmp.path().join("package");
    let build = dir.clone();
    blocking::unblock(move || prepare(&build))
        .await
        .map_err(TaskError::operation)?;
    let output = tmp.path().canonicalize().map_err(TaskError::operation)?;
    ctx.exec(
        ProcessCommand::new("npm")
            .args([
                "pack",
                "--ignore-scripts",
                "--offline",
                "--pack-destination",
            ])
            .arg(&output)
            .current_dir(&dir)
            .env_remove("NPM_TOKEN")
            .env_remove("NODE_AUTH_TOKEN"),
    )
    .await?;
    let filename = std::format!("ewe-studios-foundation-wasm-ui-{VERSION}.tgz");
    let staged = output.join(&filename);
    let dest = args.output.join(filename);
    // Node versions can compress identical npm tar streams into different bytes.
    let destination = dest.clone();
    blocking::unblock(move || -> io::Result<()> {
        let parent = destination
            .parent()
            .ok_or_else(|| io::Error::other("Missing archive directory"))?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        let source = fs::File::open(staged)?;
        let mut source = flate2::read::GzDecoder::new(source);
        let mut gzip = flate2::GzBuilder::new()
            .mtime(0)
            .operating_system(255)
            .write(file.as_file_mut(), flate2::Compression::default());
        io::copy(&mut source, &mut gzip)?;
        gzip.finish()?;
        file.as_file().sync_all()?;
        file.persist(destination).map_err(|e| e.error)?;
        Ok(())
    })
    .await
    .map_err(TaskError::operation)?;
    std::println!(
        "{} {}",
        super::npm_release::integrity(&dest).map_err(TaskError::operation)?,
        dest.display()
    );
    Ok(())
}
pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add(
        "npm-pack",
        "Build the installable browser runtime archive",
        pack,
    )?;
    set.expose("npm-pack", &["wasm-bundle", "npm-pack"])
}
