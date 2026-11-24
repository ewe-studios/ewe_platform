//! Save unconfirmed Todo requests before a host sends them.
use super::{dialog, error, todo_store, AddInput, StoreState};
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use foundation_ui_runtime::action::{ActionError, ActionResult, Context, Interaction};
use foundation_ui_wire::ErrorCode;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub(crate) mod records;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PendingChange {
    Add {
        #[serde(alias = "creation_id")]
        id: super::TodoId,
        title: String,
    },
    Delete {
        id: super::TodoId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    pub change: PendingChange,
    pub retry: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<String>,
}

type Entries = BTreeMap<String, Pending>;

struct Outbox {
    initialized: futures_util::lock::Mutex<bool>,
}

pub(super) fn install(ctx: &Ctx) {
    #[cfg(not(all(feature = "web", target_family = "wasm")))]
    ctx.store(foundation_db::MemoryStorage::new());
    ctx.store(Outbox {
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
        let mut entries = records::load(ctx).await?;
        for entry in entries.values_mut() {
            entry.retry = true;
        }
        entries
    };
    let previous = entries.clone();
    let (changed, result) = change(&mut entries)?;
    if changed {
        records::save(ctx, &previous, &entries).await?;
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

#[foundation_ui_runtime::action(host)]
pub(super) async fn submit_todo(ctx: &Context, event: Interaction<()>) -> ActionResult {
    let title = event.input_value;
    if title.trim().is_empty() {
        return Err(error("Enter a todo title"));
    }
    let env = ctx
        .get::<foundation_context::env::Env>()
        .ok_or_else(|| error("Missing Todo entropy source"))?;
    let id = env
        .entropy()
        .uuid_v4()
        .map_err(|cause| error(&cause.to_string()))?;
    submit(
        ctx,
        format!("add:{id}"),
        Some(PendingChange::Add { id, title }),
    )
    .await
}

#[foundation_ui_runtime::action(host)]
pub(super) async fn submit_delete(ctx: &Context, event: Interaction<()>) -> ActionResult {
    let id = super::todo_id(&event.input_value)?;
    submit(
        ctx,
        format!("delete:{id}"),
        Some(PendingChange::Delete { id }),
    )
    .await
}

#[foundation_ui_runtime::action(host)]
pub(super) async fn retry_todo(ctx: &Context, event: Interaction<()>) -> ActionResult {
    submit(ctx, event.input_value, None).await
}

async fn submit(ctx: &Context, key: String, proposed: Option<PendingChange>) -> ActionResult {
    let close_dialog = proposed.is_some();
    let selected = change_pending(ctx, |entries| {
        if let Some(entry) = entries.get_mut(&key) {
            if !entry.retry {
                return Ok((false, None));
            }
            entry.retry = false;
            return Ok((true, Some(entry.change.clone())));
        }
        let Some(change) = proposed else {
            return Ok((false, None));
        };
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
        return Ok(());
    };
    {
        let store = todo_store(ctx)?;
        let mut state = store
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if close_dialog {
            state.correcting = None;
            state.add_dialog = false;
            state.delete_target = None;
            state.draft.clear();
            dialog(ctx, &state.snapshot())?;
        }
        state.render_rows(ctx)?;
    }
    let result = match selected {
        PendingChange::Add { id, title } => {
            ctx.trigger_structured("server:add_todo", &AddInput { id, title })
                .await
        }
        PendingChange::Delete { id } => ctx.trigger_structured("server:confirm_delete", &id).await,
    };
    if let Err(cause) = result {
        let detail = cause.current_context();
        let validation = (detail.code == ErrorCode::InvalidInput).then(|| detail.message.clone());
        failed(ctx, &key, validation).await?;
    }
    Ok(())
}

async fn failed(ctx: &Context, key: &str, validation: Option<String>) -> ActionResult {
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

pub(super) struct Reconciled {
    pub(super) changed: bool,
    pub(super) added: bool,
}

pub(super) async fn reconcile(
    ctx: &Context,
    snapshot: bool,
) -> Result<Reconciled, ErrorTrace<ActionError>> {
    let store = todo_store(ctx)?;
    let (result, deleted) = change_pending(ctx, |entries| {
        let state = store
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let before = entries.len();
        let mut added = false;
        let mut deleted = std::collections::BTreeSet::new();
        entries.retain(|_, pending| {
            let accepted = confirmed(&state, &pending.change, snapshot);
            if let PendingChange::Delete { id } = pending.change {
                if accepted {
                    deleted.insert(id);
                }
            }
            if let PendingChange::Add { id, .. } = pending.change {
                added |= accepted && !state.retired_ids.contains(&id);
            }
            !accepted
        });
        let changed = entries.len() != before;
        Ok((changed, (Reconciled { changed, added }, deleted)))
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
    Ok(result)
}

fn confirmed(state: &StoreState, change: &PendingChange, snapshot: bool) -> bool {
    match change {
        PendingChange::Add { id, .. } => {
            state.retired_ids.contains(id) || state.items.iter().any(|row| row.id == *id)
        }
        PendingChange::Delete { id } => {
            state.removed.contains_key(id)
                || (snapshot && !state.items.iter().any(|row| row.id == *id))
        }
    }
}
