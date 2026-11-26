//! Prepare a complete headless Chromium installation for browser tests.
use crate::CiEnvironment;
use clap::Args;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult, TaskSet};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Args)]
struct Browser {
    /// Chrome for Testing build used by the browser fixtures.
    #[arg(long, default_value = "157.0.8103.51")]
    version: String,
}
pub(super) fn register(set: &mut TaskSet) -> TaskResult {
    set.add(
        "prepare-chromium",
        "Prepare a Chromium test browser",
        prepare,
    )?;
    set.expose("prepare-chromium", &["prepare-chromium"])
}
fn cached(path: &Path) -> bool {
    path.join("chrome").is_file() && path.join("icudtl.dat").is_file()
}
fn executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}
async fn prepare(ctx: TaskContext, args: Browser) -> TaskResult {
    let env = ctx.require::<CiEnvironment>()?;
    if let Some(file) = env
        .get("PRIMAL_TEST_CHROMIUM_BIN")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
    {
        println!("chromium: {}", file.display());
        return Ok(());
    }
    let cache = env
        .get("FOUNDATION_CHROMIUM_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            env.get("HOME")
                .map(|p| PathBuf::from(p).join(".cache/foundation/chromium"))
        })
        .ok_or_else(|| TaskError::operation("Set FOUNDATION_CHROMIUM_DIR or HOME"))?;
    if cached(&cache) {
        println!("chromium: {}", cache.join("chrome").display());
        return Ok(());
    }
    if let Some(path) = env.get("PATH") {
        for name in ["chromium", "google-chrome", "chromium-browser", "chrome"] {
            if let Some(file) = std::env::split_paths(&path)
                .map(|p| p.join(name))
                .find(|p| executable(p))
            {
                println!("chromium: {}", file.display());
                return Ok(());
            }
        }
    }
    let platform = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "mac-arm64",
        ("macos", _) => "mac-x64",
        ("linux", _) => "linux64",
        _ => {
            return Err(TaskError::operation(
                "Set PRIMAL_TEST_CHROMIUM_BIN to an installed browser",
            ));
        }
    };
    if args.version.split('.').count() != 4
        || !args
            .version
            .split('.')
            .all(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(TaskError::operation("Expected a four-part Chrome version"));
    }
    let parent = cache
        .parent()
        .ok_or_else(|| TaskError::operation("Expected a browser cache directory"))?;
    fs::create_dir_all(parent).map_err(TaskError::operation)?;
    let stage = tempfile::tempdir_in(parent).map_err(TaskError::operation)?;
    let archive = stage.path().join("chrome.zip");
    let name = format!("chrome-headless-shell-{platform}");
    let url = format!(
        "https://storage.googleapis.com/chrome-for-testing-public/{}/{platform}/{name}.zip",
        args.version
    );
    ctx.exec(
        ProcessCommand::new("curl")
            .args(["-fsSL", &url, "-o"])
            .arg(&archive),
    )
    .await?;
    ctx.exec(
        ProcessCommand::new("unzip")
            .arg("-q")
            .arg(&archive)
            .arg("-d")
            .arg(stage.path()),
    )
    .await?;
    let source = stage.path().join(name);
    let binary = source.join("chrome");
    fs::rename(source.join("chrome-headless-shell"), &binary).map_err(TaskError::operation)?;
    if !cached(&source) {
        return Err(TaskError::operation("Incomplete Chromium archive"));
    }
    ctx.exec(ProcessCommand::new(&binary).arg("--version"))
        .await?;
    install_directory(parent, &cache, stage.path(), &source)
}

fn install_directory(parent: &Path, cache: &Path, stage: &Path, source: &Path) -> TaskResult {
    let lock_path = parent.join(format!(
        ".{}.lock",
        cache.file_name().unwrap_or_default().to_string_lossy()
    ));
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path)
        .map_err(TaskError::operation)?;
    lock.lock().map_err(TaskError::operation)?;
    if cached(cache) {
        return Ok(());
    }
    let previous = stage.join("previous");
    if cache.exists() {
        fs::rename(cache, &previous).map_err(TaskError::operation)?;
    }
    if let Err(error) = fs::rename(source, cache) {
        if previous.exists() {
            fs::rename(previous, cache).map_err(TaskError::operation)?;
        }
        return Err(TaskError::operation(error));
    }
    println!("chromium: {}", cache.join("chrome").display());
    Ok(())
}
