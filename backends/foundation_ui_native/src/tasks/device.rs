//! Read device settings and supervise platform tool queries.
use super::output::query;
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use std::{collections::BTreeMap, ffi::OsString, time::Duration};

#[derive(Clone, Default)]
pub struct Options {
    vars: BTreeMap<&'static str, OsString>,
}
impl Options {
    pub(super) fn from_host() -> Self {
        let vars = [
            "ANDROID_HOME",
            "EWE_ANDROID_AUDIO",
            "EWE_ANDROID_MUTE",
            "EWE_ANDROID_SERIAL",
            "EWE_IOS_DEVICE",
            "EWE_SIMCTL_BIN",
        ]
        .into_iter()
        .filter_map(|name| {
            std::env::var_os(name)
                .filter(|s| !s.is_empty())
                .map(|v| (name, v))
        })
        .collect();
        Self { vars }
    }
    pub fn extend_from_host(&mut self, keys: &[&'static str]) {
        self.vars.extend(
            keys.iter()
                .filter_map(|&key| std::env::var_os(key).map(|value| (key, value))),
        );
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&std::ffi::OsStr> {
        self.vars.get(key).map(OsString::as_os_str)
    }
}

/// Return whether a task-created Android emulator should expose audio output.
///
/// Audio is enabled by default. Set `EWE_ANDROID_MUTE=1` or
/// `EWE_ANDROID_AUDIO=0` to disable emulator audio.
///
/// # Errors
/// Returns an error when an audio setting is not a boolean value.
pub fn android_audio_enabled(options: &Options) -> Result<bool, TaskError> {
    let value = options
        .get("EWE_ANDROID_MUTE")
        .map_or_else(|| options.get("EWE_ANDROID_AUDIO"), Some);
    let Some(value) = value else { return Ok(true) };
    let value = value
        .to_str()
        .ok_or_else(|| TaskError::operation("Invalid Android audio setting"))?;
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(options.get("EWE_ANDROID_MUTE").is_none()),
        "0" | "false" | "no" | "off" => Ok(options.get("EWE_ANDROID_MUTE").is_some()),
        _ => Err(TaskError::operation(
            "Android audio setting must be boolean",
        )),
    }
}

/// Apply the shared Android emulator audio policy to a command.
#[must_use]
pub fn with_android_audio(command: ProcessCommand, enabled: bool) -> ProcessCommand {
    if enabled {
        command
    } else {
        command.arg("-no-audio")
    }
}

/// Read bounded text from a host command.
///
/// # Errors
/// Returns command failures and invalid UTF-8 output.
pub async fn text(ctx: &TaskContext, cmd: ProcessCommand) -> Result<String, TaskError> {
    Ok(String::from_utf8(query(ctx, cmd).await?)
        .map_err(TaskError::operation)?
        .trim()
        .to_string())
}

/// Wait until the deadline or task cancellation.
///
/// # Errors
/// Returns cancellation, clock range, or missing timer errors.
pub async fn sleep(ctx: &TaskContext, duration: Duration) -> TaskResult {
    let host = ctx.context();
    let deadline = host
        .clock()
        .now()
        .checked_add(duration)
        .ok_or_else(|| TaskError::operation("Timer deadline exceeds the clock range"))?;
    let timer = host.require_env().map_err(TaskError::operation)?.timer();
    let wait = async {
        timer.sleep_until(host.clone(), deadline).await;
        Ok(())
    };
    let cancelled = async {
        ctx.cancellation().cancelled().await;
        Err(TaskError::Cancelled)
    };
    futures_lite::future::race(wait, cancelled).await
}
