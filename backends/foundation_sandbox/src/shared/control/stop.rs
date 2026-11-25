//! The stop and kill stories and the watcher of a ready sandbox.

use std::sync::Arc;

use foundation_core::valtron::{from_future, send};

use crate::shared::control::wait_until;
use futures::future::{select, Either};

use crate::shared::control::manager::Inner;
use crate::shared::control::record::SandboxRecord;
use crate::shared::control::ExitReport;
use crate::shared::control::SandboxState;
use crate::shared::driver::{SubstrateExit, SubstrateHandle};
use crate::shared::error::{SandboxError, SandboxResult};

/// Starts one background future and does not wait for it.
///
/// The task runs on the shared executor. A failure to schedule it is logged;
/// the caller cannot do more at that point.
pub fn spawn_detached<F>(future: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    if let Err(error) = send(from_future(future)) {
        tracing::warn!(error = %error, "could not start a background task");
    }
}

/// Stops one sandbox: `Shutdown`, the grace wait, then force if needed.
///
/// # Errors
/// [`SandboxError::SandboxStopped`] when the sandbox is still starting, or
/// [`SandboxError::Io`] when the clean-up failed. The first `ExitReport` is
/// kept for later calls.
pub async fn stop_record(
    inner: Arc<Inner>,
    record: Arc<SandboxRecord>,
) -> SandboxResult<ExitReport> {
    let mut wait_for_stop = false;
    let (substrate, client) = {
        let mut state = record
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match state.state {
            SandboxState::Stopped | SandboxState::Failed => {
                let report = record.stop.report().or(state.exit).unwrap_or_default();
                return Ok(report);
            }
            SandboxState::Stopping => {
                wait_for_stop = true;
                (None, None)
            }
            SandboxState::Creating => return Err(SandboxError::SandboxStopped.into()),
            SandboxState::Ready => {
                state.state = SandboxState::Stopping;
                (state.substrate.clone(), state.client.clone())
            }
        }
    };
    if wait_for_stop {
        return Ok(record.stop.wait().await);
    }
    inner.persist(&record).await;

    let grace = record.spec.limits.shutdown_grace;
    let deadline = inner.ctx.clock().now() + grace;
    if let Some(client) = &client {
        let grace_ms = u32::try_from(grace.as_millis()).unwrap_or(u32::MAX);
        // A failed `Shutdown` is not an error: the slave may already be gone.
        let _ = client.shutdown(grace_ms).await;
    }

    let report = if let Some(substrate) = &substrate {
        let exit = wait_for_exit(substrate, deadline, &inner).await;
        if let Some(exit) = exit {
            ExitReport {
                substrate_exit_code: exit.code,
                killed: false,
            }
        } else {
            let forced = force_exit(substrate, &inner).await;
            ExitReport {
                substrate_exit_code: forced.code,
                killed: true,
            }
        }
    } else {
        ExitReport::default()
    };

    let cleanup = match &substrate {
        Some(substrate) => Some(substrate.cleanup().await),
        None => None,
    };
    finish(&inner, &record, report).await;
    match cleanup {
        Some(Err(error)) => Err(SandboxError::Io {
            op: "cleanup".to_string(),
            message: error.to_string(),
        }
        .into()),
        _ => Ok(report),
    }
}

/// Kills one sandbox and cleans up.
///
/// # Errors
/// The error of the clean-up.
pub async fn kill_record(
    inner: Arc<Inner>,
    record: Arc<SandboxRecord>,
) -> SandboxResult<ExitReport> {
    let mut wait_for_stop = false;
    let substrate = {
        let mut state = record
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match state.state {
            SandboxState::Stopped | SandboxState::Failed => {
                let report = record.stop.report().or(state.exit).unwrap_or_default();
                return Ok(report);
            }
            SandboxState::Stopping => {
                wait_for_stop = true;
                None
            }
            SandboxState::Creating | SandboxState::Ready => {
                state.state = SandboxState::Stopping;
                state.substrate.clone()
            }
        }
    };
    if wait_for_stop {
        return Ok(record.stop.wait().await);
    }
    let exit = match &substrate {
        Some(substrate) => substrate.kill().await.ok(),
        None => None,
    };
    let cleanup = match &substrate {
        Some(substrate) => Some(substrate.cleanup().await),
        None => None,
    };
    let report = ExitReport {
        substrate_exit_code: exit.and_then(|exit| exit.code),
        killed: true,
    };
    finish(&inner, &record, report).await;
    match cleanup {
        Some(Err(error)) => Err(SandboxError::Io {
            op: "cleanup".to_string(),
            message: error.to_string(),
        }
        .into()),
        _ => Ok(report),
    }
}

/// Waits for the substrate exit, up to `grace`.
async fn wait_for_exit(
    substrate: &Arc<dyn SubstrateHandle>,
    deadline: foundation_compact::Instant,
    inner: &Arc<Inner>,
) -> Option<SubstrateExit> {
    let sleep = Box::pin(async move {
        let _ = wait_until(deadline, &inner.ctx).await;
    });
    match select(substrate.exited(), sleep).await {
        Either::Left((exit, _sleep)) => Some(exit),
        Either::Right(((), _exit)) => None,
    }
}

/// Forces the substrate down, with `terminate` and then `kill`.
async fn force_exit(substrate: &Arc<dyn SubstrateHandle>, inner: &Arc<Inner>) -> SubstrateExit {
    match substrate.terminate(inner.config.terminate_grace).await {
        Ok(exit) => exit,
        Err(_) => substrate.kill().await.unwrap_or(SubstrateExit {
            code: None,
            signal: Some(9),
            forced: true,
        }),
    }
}

/// Moves the record to `Stopped`, removes it and keeps the report.
async fn finish(inner: &Arc<Inner>, record: &Arc<SandboxRecord>, report: ExitReport) {
    {
        let mut state = record
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.exit = Some(report);
        state.state = SandboxState::Stopped;
    }
    inner.remove(&record.id);
    inner.remember_exit(record.id.clone(), report);
    record.stop.complete(report);
    if let Err(error) = inner.store.remove(&record.id).await {
        tracing::warn!(error = %error, id = %record.id, "the record store did not remove the record");
    }
}

/// Watches a ready sandbox: a substrate exit moves it to `Failed`, then
/// `Stopped`, and cleans up.
pub fn start_watcher(inner: Arc<Inner>, record: Arc<SandboxRecord>) {
    let substrate = {
        let state = record
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.substrate.clone()
    };
    let Some(substrate) = substrate else {
        return;
    };
    spawn_detached(async move {
        let exit = substrate.exited().await;
        {
            let mut state = record
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.state != SandboxState::Ready {
                return;
            }
            state.state = SandboxState::Failed;
        }
        inner.persist(&record).await;
        let cleanup = substrate.cleanup().await;
        let report = ExitReport {
            substrate_exit_code: exit.code,
            killed: false,
        };
        {
            let mut state = record
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.exit = Some(report);
            state.state = SandboxState::Stopped;
        }
        inner.remove(&record.id);
        inner.remember_exit(record.id.clone(), report);
        record.stop.complete(report);
        if let Err(error) = inner.store.remove(&record.id).await {
            tracing::warn!(error = %error, id = %record.id, "the record store did not remove the failed record");
        }
        if let Err(error) = cleanup {
            tracing::warn!(error = %error, id = %record.id, "the driver could not clean up a failed sandbox");
        }
    });
}
