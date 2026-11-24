//! Execute a required interpreter script from a package's embedded directory.
use crate::{ProcessCommand, TaskContext, TaskError, TaskResult};
use foundation_nostd::embeddable::{DataCompression, EmbeddableDirectory};
use std::{
    ffi::OsString,
    fs,
    path::{Component, Path},
};

/// Platform where the script executes. A caller selects the host or the target device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptPlatform {
    Linux,
    Macos,
    Windows,
    Android,
    Ios,
}
impl ScriptPlatform {
    /// Resolve the current execution host.
    ///
    /// # Errors
    /// Returns an error on an unsupported host.
    pub fn host() -> Result<Self, TaskError> {
        match std::env::consts::OS {
            "linux" => Ok(Self::Linux),
            "macos" => Ok(Self::Macos),
            "windows" => Ok(Self::Windows),
            "android" => Ok(Self::Android),
            "ios" => Ok(Self::Ios),
            host => Err(TaskError::operation(format!(
                "Unsupported script host {host}"
            ))),
        }
    }
}

/// A script entry relative to the embedded root. A portable entry uses `None`.
pub struct ScriptVariant<'a> {
    pub platform: Option<ScriptPlatform>,
    pub entry: &'a str,
}

fn relative(path: &str) -> bool {
    let path = Path::new(path);
    !path.as_os_str().is_empty() && path.components().all(|c| matches!(c, Component::Normal(_)))
}
/// Stage companion files, execute the selected entry, and remove the temporary directory.
///
/// # Errors
/// Returns unsupported platform, invalid embedded path, process, cancellation, and cleanup errors.
pub async fn run<D: EmbeddableDirectory>(
    ctx: &TaskContext,
    files: &D,
    variants: &[ScriptVariant<'_>],
    platform: ScriptPlatform,
    interpreter: ProcessCommand,
    arguments: &[OsString],
) -> TaskResult {
    let entry = variants
        .iter()
        .find(|v| v.platform == Some(platform))
        .or_else(|| variants.iter().find(|v| v.platform.is_none()))
        .ok_or_else(|| TaskError::operation(format!("No script supports {platform:?}")))?
        .entry;
    if !relative(entry)
        || !D::FILES_METADATA
            .iter()
            .any(|file| !file.is_directory && file.source_path == entry)
    {
        return Err(TaskError::operation(format!(
            "Missing embedded script {entry}"
        )));
    }
    if !matches!(files.compression(), DataCompression::NONE) {
        return Err(TaskError::operation(
            "Script directories must use uncompressed embedded bytes",
        ));
    }
    let directory = blocking::unblock(|| {
        let temporary = tempfile::tempdir().map_err(TaskError::operation)?;
        for file in D::FILES_METADATA.iter().filter(|f| !f.is_directory) {
            if !relative(file.source_path) {
                return Err(TaskError::operation("Invalid embedded companion path"));
            }
            let path = temporary.path().join(file.source_path);
            let parent = path
                .parent()
                .ok_or_else(|| TaskError::operation("Missing embedded file parent"))?;
            fs::create_dir_all(parent).map_err(TaskError::operation)?;
            let bytes = D::FILES_DATA
                .iter()
                .find(|entry| entry.1 == file.source_path)
                .map(|e| e.3)
                .ok_or_else(|| {
                    TaskError::operation(format!("Missing embedded data for {}", file.source_path))
                })?;
            fs::write(path, bytes).map_err(TaskError::operation)?;
        }
        Ok::<_, TaskError>(temporary)
    })
    .await?;
    let command = interpreter
        .arg(directory.path().join(entry))
        .args(arguments.iter().cloned());
    let result = ctx.exec(command).await;
    let cleanup = blocking::unblock(move || directory.close())
        .await
        .map_err(TaskError::operation);
    match (result, cleanup) {
        (Err(source), Err(cleanup)) => Err(TaskError::Cleanup {
            source: Box::new(source),
            cleanup: Box::new(cleanup),
        }),
        (Err(e), _) | (_, Err(e)) => Err(e),
        _ => Ok(()),
    }
}
