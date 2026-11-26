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
