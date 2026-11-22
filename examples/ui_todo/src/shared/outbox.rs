//! Save unconfirmed Todo requests before a host sends them.
use super::{add_todo, confirm_delete, dialog, error, todo_store, AddInput, StoreState};
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use foundation_ui_runtime::action::{Action, ActionError, ActionResult, Context};
use foundation_ui_wire::{finish_frame, read_frame, FrameKindT, WireLimits};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PendingChange {
    Add { creation_id: String, title: String },
    Delete { id: u64 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pending {
    pub change: PendingChange,
    pub retry: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<String>,
}

type Entries = BTreeMap<String, Pending>;

#[async_trait::async_trait]
pub trait PendingStorage: Send + Sync {
    async fn load(&self) -> Result<Vec<u8>, String>;
    async fn save(&self, contents: Vec<u8>) -> Result<(), String>;
}

#[derive(Default)]
pub(super) struct MemoryStorage(Mutex<Vec<u8>>);

#[async_trait::async_trait]
impl PendingStorage for MemoryStorage {
    async fn load(&self) -> Result<Vec<u8>, String> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }
    async fn save(&self, contents: Vec<u8>) -> Result<(), String> {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = contents;
        Ok(())
    }
}

struct Outbox {
    storage: Arc<dyn PendingStorage>,
    initialized: futures_util::lock::Mutex<bool>,
}

/// Install the storage selected by this host.
pub fn install_pending_storage(ctx: &Ctx, storage: Arc<dyn PendingStorage>) {
    ctx.store(Outbox {
        storage,
        initialized: futures_util::lock::Mutex::new(false),
    });
}

async fn change_pending<T>(
    ctx: &Context,
    change: impl FnOnce(&mut Entries) -> Result<(bool, T), ErrorTrace<ActionError>>,
) -> Result<T, ErrorTrace<ActionError>> {
    let store = todo_store(ctx)?;
    let outbox = ctx
        .get::<Outbox>()
        .ok_or_else(|| error("Missing Todo pending storage"))?;
    let mut initialized = outbox.initialized.lock().await;
    let mut entries = if *initialized {
        store
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pending
            .clone()
    } else {
        let bytes = outbox.storage.load().await.map_err(|e| error(&e))?;
        if bytes.is_empty() {
            Entries::new()
        } else {
            let mut entries: Entries =
                serde_json::from_slice(&bytes).map_err(|e| error(&e.to_string()))?;
            for entry in entries.values_mut() {
                entry.retry = true;
            }
            entries
        }
    };
    let (changed, result) = change(&mut entries)?;
    if changed {
        outbox
            .storage
            .save(serde_json::to_vec(&entries).map_err(|e| error(&e.to_string()))?)
            .await
            .map_err(|e| error(&e))?;
    }
    store
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .pending = entries;
    *initialized = true;
    Ok(result)
}

pub(super) async fn restore(ctx: &Context) -> ActionResult {
    change_pending(ctx, |_| Ok((false, ()))).await
}

pub(crate) struct Prepared {
    pub(crate) request: Option<Vec<u8>>,
    pub(crate) key: Option<String>,
}

pub(crate) async fn prepare(
    ctx: &Context,
    bytes: Vec<u8>,
) -> Result<Prepared, ErrorTrace<ActionError>> {
    let frame = read_frame(&bytes, &WireLimits::default()).map_err(|e| error(&e.to_string()))?;
    let Some(action) = frame
        .kind_as_frame_request()
        .and_then(|request| request.action())
    else {
        return Ok(Prepared {
            request: Some(bytes),
            key: None,
        });
    };
    let name = action.action().unwrap_or_default();
    let add = format!("server:{}", add_todo::NAME);
    let delete = format!("server:{}", confirm_delete::NAME);
    if name != add && name != delete {
        return Ok(Prepared {
            request: Some(bytes),
            key: None,
        });
    }
    let mut packet = frame.unpack();
    let FrameKindT::FrameRequest(request) = &mut packet.kind else {
        return Err(error("Expected an action request"));
    };
    let input = request
        .action
        .as_mut()
        .ok_or_else(|| error("Missing Todo action"))?
        .input
        .get_or_insert_with(Default::default);
    let value = input.value.as_deref().unwrap_or_default();
    let (key, proposed) = if name == add {
        if let Some(key) = value
            .strip_prefix("retry-add:")
            .filter(|_| input.event_name.as_deref() != Some("submit"))
        {
            (format!("add:{key}"), None)
        } else {
            if value.trim().is_empty() {
                return Err(error("Enter a todo title"));
            }
            let mut bytes = [0u8; 16];
            ctx.inner()
                .get::<foundation_context::env::Env>()
                .ok_or_else(|| error("Missing Todo entropy source"))?
                .entropy()
                .fill(&mut bytes)
                .map_err(|e| error(&e.to_string()))?;
            bytes[6] = bytes[6] & 0x0f | 0x40;
            bytes[8] = bytes[8] & 0x3f | 0x80;
            let creation_id = foundation_compact::uuid::Uuid::from_bytes(bytes).to_string();
            (
                format!("add:{creation_id}"),
                Some(PendingChange::Add {
                    creation_id,
                    title: value.into(),
                }),
            )
        }
    } else {
        let id = super::todo_id(value)?;
        (format!("delete:{id}"), Some(PendingChange::Delete { id }))
    };
    let selected = change_pending(ctx, |entries| {
        if let Some(entry) = entries.get_mut(&key) {
            if !entry.retry {
                return Ok((false, None));
            }
            entry.retry = false;
            return Ok((true, Some(entry.change.clone())));
        }
        let change = proposed.ok_or_else(|| error("The pending addition has already finished"))?;
        if let PendingChange::Add { .. } = change {
            let correction = todo_store(ctx)?
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .correcting
                .clone();
            if let Some(previous) = correction {
                entries.remove(&previous);
            }
        }
        entries.insert(
            key.clone(),
            Pending {
                change: change.clone(),
                retry: false,
                validation: None,
            },
        );
        Ok((true, Some(change)))
    })
    .await?;
    let Some(selected) = selected else {
        return Ok(Prepared {
            request: None,
            key: None,
        });
    };
    if let PendingChange::Add { creation_id, title } = selected {
        input.value = None;
        input.json = Some(
            serde_json::to_vec(&AddInput {
                creation_id: Some(creation_id),
                title,
            })
            .map_err(|e| error(&e.to_string()))?,
        );
    }
    let store = todo_store(ctx)?;
    let mut state = store
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state.correcting = None;
    state.add_dialog = false;
    state.delete_target = None;
    state.draft.clear();
    dialog(ctx, &state.snapshot())?;
    state.render_rows(ctx)?;
    Ok(Prepared {
        request: Some(finish_frame(&packet).map_err(|e| error(&e.to_string()))?),
        key: Some(key),
    })
}

pub(crate) async fn failed(ctx: &Context, key: &str, validation: Option<String>) -> ActionResult {
    let changed = change_pending(ctx, |entries| {
        let Some(entry) = entries.get_mut(key) else {
            return Ok((false, false));
        };
        entry.retry = true;
        if validation.is_some() && matches!(entry.change, PendingChange::Add { .. }) {
            entry.validation = validation;
        }
        Ok((true, true))
    })
    .await?;
    if changed {
        let store = todo_store(ctx)?;
        let state = store
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.render_rows(ctx)?;
    }
    Ok(())
}

pub(super) async fn reconcile(
    ctx: &Context,
    snapshot: bool,
) -> Result<bool, ErrorTrace<ActionError>> {
    let store = todo_store(ctx)?;
    let (changed, deleted) = change_pending(ctx, |entries| {
        let state = store
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let before = entries.len();
        let mut deleted = std::collections::BTreeSet::new();
        entries.retain(|_, pending| {
            let accepted = confirmed(&state, &pending.change, snapshot);
            if let PendingChange::Delete { id } = pending.change {
                if accepted {
                    deleted.insert(id);
                }
            }
            !accepted
        });
        let changed = entries.len() != before;
        Ok((changed, (changed, deleted)))
    })
    .await?;
    if !deleted.is_empty() {
        let mut state = store
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(rows) = &mut state.search_rows {
            rows.retain(|row| !deleted.contains(&row.id));
        }
    }
    Ok(changed)
}

fn confirmed(state: &StoreState, change: &PendingChange, snapshot: bool) -> bool {
    match change {
        PendingChange::Add { creation_id, .. } => {
            state.retired_creations.contains(creation_id)
                || state
                    .items
                    .iter()
                    .any(|row| row.creation_id.as_ref() == Some(creation_id))
        }
        PendingChange::Delete { id } => {
            state.removed.contains_key(id)
                || (snapshot && !state.items.iter().any(|row| row.id == *id))
        }
    }
}

/// The trusted browser adapter receives the rewritten request before sending it.
pub(crate) struct BrowserReply(pub(crate) fn(u32, &Prepared));
#[derive(Deserialize)]
pub(super) struct BrowserRequest {
    call: u32,
    bytes: Vec<u8>,
}

#[foundation_ui_runtime::action(host, web)]
pub(super) async fn prepare_todo_request(
    ctx: &Context,
    event: foundation_ui_runtime::action::Interaction<(), BrowserRequest>,
) -> ActionResult {
    let prepared = prepare(ctx, event.input_value.bytes).await?;
    let reply = ctx
        .get::<BrowserReply>()
        .ok_or_else(|| error("Missing browser request adapter"))?;
    (reply.0)(event.input_value.call, &prepared);
    Ok(())
}

#[derive(Deserialize)]
pub(super) struct FailedRequest {
    key: String,
    validation: Option<String>,
}

#[foundation_ui_runtime::action(host, web)]
pub(super) async fn uncertain_todo_request(
    ctx: &Context,
    event: foundation_ui_runtime::action::Interaction<(), FailedRequest>,
) -> ActionResult {
    failed(ctx, &event.input_value.key, event.input_value.validation).await
}
