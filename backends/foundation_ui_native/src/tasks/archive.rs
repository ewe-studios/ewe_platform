//! Write native release archives in a stable order.
use foundation_tasks::{TaskError, TaskResult};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    fs::File,
    io,
    path::{Path, PathBuf},
};
use zip::{write::SimpleFileOptions, CompressionMethod, DateTime, ZipWriter};

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Framework {
    available_libraries: Vec<Library>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Library {
    #[serde(rename = "SupportedPlatform")]
    platform: String,
    #[serde(rename = "SupportedPlatformVariant")]
    variant: Option<String>,
    #[serde(rename = "SupportedArchitectures")]
    architectures: BTreeSet<String>,
}

pub(super) fn apple(layout: &Path) -> TaskResult {
    let bundle = layout.join("EweCoreFFI.xcframework");
    let framework: Framework =
        plist::from_file(bundle.join("Info.plist")).map_err(TaskError::operation)?;
    let mut slices = BTreeSet::new();
    for lib in framework.available_libraries {
        slices.insert((lib.platform, lib.variant, lib.architectures));
    }
    let arm = String::from("arm64");
    let both = BTreeSet::from([arm.clone(), String::from("x86_64")]);
    let expected = BTreeSet::from([
        ("ios".into(), None, BTreeSet::from([arm])),
        ("ios".into(), Some("simulator".into()), both.clone()),
        ("macos".into(), None, both),
    ]);
    if slices != expected {
        return Err(TaskError::operation("Unexpected XCFramework slices"));
    }
    let mut files = Vec::new();
    collect(&bundle, &mut files).map_err(TaskError::operation)?;
    pack(layout, files, &layout.join("EweCoreFFI.xcframework.zip"))
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in dir.read_dir()? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect(&entry.path(), files)?;
        } else if kind.is_file() {
            files.push(entry.path());
        }
    }
    Ok(())
}

pub(super) fn pack(base: &Path, mut files: Vec<PathBuf>, output: &Path) -> TaskResult {
    files.sort();
    let file = File::create(output).map_err(TaskError::operation)?;
    let mut writer = ZipWriter::new(file);
    for path in files {
        let mut source = File::open(&path).map_err(TaskError::operation)?;
        let name = path.strip_prefix(base).map_err(TaskError::operation)?;
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(6))
            .last_modified_time(DateTime::default())
            .unix_permissions(0o644)
            .large_file(
                source.metadata().map_err(TaskError::operation)?.len() >= u64::from(u32::MAX),
            );
        writer
            .start_file_from_path(name, options)
            .map_err(TaskError::operation)?;
        io::copy(&mut source, &mut writer).map_err(TaskError::operation)?;
    }
    writer.finish().map_err(TaskError::operation)?;
    Ok(())
}

#[derive(clap::Args)]
pub(super) struct Archive {
    /// Directory that contains EweCoreFFI.xcframework.
    #[arg(long)]
    stage: PathBuf,
}

pub(super) async fn run(ctx: foundation_tasks::TaskContext, args: Archive) -> TaskResult {
    let root = ctx.require::<super::NativeBuildEnv>()?.root().await?;
    blocking::unblock(move || apple(&root.join(args.stage))).await
}
