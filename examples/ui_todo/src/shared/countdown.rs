//! Keep each Todo input pause scoped to its initiating session.
use super::TodoId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Scope {
    session: TodoId,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Request {
    #[serde(flatten)]
    scope: Scope,
    #[serde(rename = "request")]
    operation_id: TodoId,
    issued_at_ms: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Current {
    #[serde(flatten)]
    scope: Scope,
    observed: Option<TodoId>,
    #[serde(default)]
    await_release: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Block {
    id: TodoId,
    expires_at_ms: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) enum Update {
    Active {
        scope: Scope,
        block: Block,
        remaining_ms: u64,
    },
    Receipt {
        scope: Scope,
        block: Block,
        request: TodoId,
    },
    Released {
        scope: Scope,
        id: TodoId,
    },
    Clear {
        scope: Scope,
        observed: Option<TodoId>,
    },
}

fn wall_ms(ctx: &foundation_ui_runtime::action::Context) -> u64 {
    u64::try_from(foundation_compact::time::unix_millis(
        ctx.clock().wall_now(),
    ))
    .unwrap_or_default()
}

#[cfg(not(target_family = "wasm"))]
mod service;
#[cfg(all(
    any(feature = "server", feature = "mobile"),
    not(target_family = "wasm")
))]
pub(crate) use service::install;
#[cfg(all(feature = "server", not(target_family = "wasm")))]
pub use service::CountdownConfig;
#[cfg(not(target_family = "wasm"))]
pub(super) use service::{current_countdown, request_countdown};

#[cfg(target_family = "wasm")]
#[foundation_ui_runtime::action(server)]
pub(super) async fn request_countdown(
    _ctx: &foundation_ui_runtime::action::Context,
    _event: foundation_ui_runtime::action::Interaction<(), Request>,
) -> foundation_ui_runtime::action::ActionResult {
    Ok(())
}

#[cfg(target_family = "wasm")]
#[foundation_ui_runtime::action(server)]
pub(super) async fn current_countdown(
    _ctx: &foundation_ui_runtime::action::Context,
    _event: foundation_ui_runtime::action::Interaction<(), Current>,
) -> foundation_ui_runtime::action::ActionResult {
    Ok(())
}

#[cfg(all(feature = "mobile", not(target_family = "wasm")))]
pub(crate) struct NativeCountdownLease {
    _file: std::fs::File,
}

#[cfg(all(feature = "mobile", not(target_family = "wasm")))]
impl NativeCountdownLease {
    pub(crate) fn claim(path: &std::path::Path) -> std::io::Result<Option<Self>> {
        let path = path.with_added_extension("countdown.lock");
        if let Some(directory) = path
            .parent()
            .filter(|directory| !directory.as_os_str().is_empty())
        {
            std::fs::create_dir_all(directory)?;
        }
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(std::fs::TryLockError::WouldBlock) => Ok(None),
            Err(std::fs::TryLockError::Error(cause)) => Err(cause),
        }
    }
}

mod host;

foundation_ui_runtime::__ewe_action_host! {
    @items
    pub(crate) use host::install_host;
    pub(super) use host::{apply, restore_view};
}
pub(super) use host::Tick;
pub(super) use host::{button, pause_todo_input, tick_todo_countdown};
