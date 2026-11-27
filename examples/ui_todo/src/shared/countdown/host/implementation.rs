//! Keep the local countdown state and reconcile it with server replies.
use super::{button, pause_todo_input, Tick};
use crate::shared::countdown::{wall_ms, Block, Current, Request, Scope, TodoId, Update};
use crate::shared::{error, replace};
use foundation_context::{env::Env, Ctx, Instant};
use foundation_errstacks::ErrorTrace;
use foundation_ui_runtime::{
    action::{ActionError, ActionResult, Context},
    html,
    view::template_types::Html,
};
use foundation_ui_wire::ErrorCode;
use futures_util::lock::Mutex;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Session {
    scope: Scope,
    pending: Option<Attempt>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Attempt {
    id: TodoId,
    issued_at_ms: u64,
}

struct Active {
    block: Block,
    deadline: Instant,
    checking: bool,
}

#[derive(Default)]
struct State {
    session: Option<Session>,
    busy: bool,
    retry: bool,
    expired_request: bool,
    active: Option<Active>,
    initialized: bool,
    released: Option<TodoId>,
}

type ActionValue<T> = Result<T, ErrorTrace<ActionError>>;

pub(crate) fn install_host(ctx: &Ctx) {
    ctx.store(Mutex::new(State::default()));
}

fn state(ctx: &Context) -> ActionValue<Arc<Mutex<State>>> {
    ctx.get::<Mutex<State>>()
        .ok_or_else(|| error("Missing countdown display state"))
}

fn fresh_id(ctx: &Context) -> ActionValue<TodoId> {
    let env = ctx
        .get::<Env>()
        .ok_or_else(|| error("Missing countdown identifier source"))?;
    env.entropy()
        .uuid_v4()
        .map_err(|cause| error(&cause.to_string()))
}

fn key(session: Option<TodoId>) -> String {
    session.map_or_else(
        || "todo/countdown".into(),
        |id| format!("todo/countdown/{id}"),
    )
}

async fn load(ctx: &Context, external_session: Option<TodoId>) -> ActionValue<Session> {
    let key = key(external_session);
    let stored = read(ctx, &key).await?;
    if let Some(stored) = stored {
        return Ok(stored);
    }
    let session = Session {
        scope: Scope {
            session: match external_session {
                Some(id) => id,
                None => fresh_id(ctx)?,
            },
        },
        pending: None,
    };
    if external_session.is_none() {
        write(ctx, &key, &session).await?;
    }
    Ok(session)
}

#[cfg(all(feature = "mobile", not(target_family = "wasm")))]
fn native_file(ctx: &Context) -> Option<Arc<foundation_ui_native::storage::NativeRecordFile>> {
    ctx.get::<crate::shared::countdown::NativeCountdownLease>()?;
    ctx.get::<foundation_ui_native::storage::NativeRecordFile>()
}

async fn read(ctx: &Context, key: &str) -> ActionValue<Option<Session>> {
    #[cfg(all(feature = "web", target_family = "wasm"))]
    {
        use foundation_wasm_ui::storage::{HostStorage, KeyValueStore};
        let store = ctx
            .get::<HostStorage>()
            .ok_or_else(|| error("Missing browser storage"))?;
        store
            .get(key)
            .await
            .map_err(|cause| error(&cause.to_string()))
    }
    #[cfg(not(all(feature = "web", target_family = "wasm")))]
    {
        use foundation_db::{KeyValueStore, MemoryStorage};
        #[cfg(all(feature = "mobile", not(target_family = "wasm")))]
        if let Some(file) = native_file(ctx) {
            let db = file
                .database(ctx.inner())
                .await
                .map_err(|cause| error(&cause.to_string()))?;
            return db.get(key).await.map_err(|cause| error(&cause.to_string()));
        }
        ctx.get::<MemoryStorage>()
            .ok_or_else(|| error("Missing countdown storage"))?
            .get(key)
            .await
            .map_err(|cause| error(&cause.to_string()))
    }
}

async fn write(ctx: &Context, key: &str, session: &Session) -> ActionResult {
    #[cfg(all(feature = "web", target_family = "wasm"))]
    {
        use foundation_wasm_ui::storage::{HostStorage, KeyValueStore};
        let store = ctx
            .get::<HostStorage>()
            .ok_or_else(|| error("Missing browser storage"))?;
        if session.pending.is_some() {
            store.set(key, session.clone()).await
        } else {
            store.delete(key).await
        }
        .map_err(|cause| error(&cause.to_string()))
    }
    #[cfg(not(all(feature = "web", target_family = "wasm")))]
    {
        use foundation_db::{KeyValueStore, MemoryStorage};
        #[cfg(all(feature = "mobile", not(target_family = "wasm")))]
        if let Some(file) = native_file(ctx) {
            let db = file
                .database(ctx.inner())
                .await
                .map_err(|cause| error(&cause.to_string()))?;
            return db
                .set(key, session.clone())
                .await
                .map_err(|cause| error(&cause.to_string()));
        }
        ctx.get::<MemoryStorage>()
            .ok_or_else(|| error("Missing countdown storage"))?
            .set(key, session.clone())
            .await
            .map_err(|cause| error(&cause.to_string()))
    }
}

fn show_controls(ctx: &Context, state: &State) -> ActionResult {
    let notice = if state.busy {
        Some(html! { <Text>{"Pausing input…"}</Text> })
    } else if state.retry {
        Some(html! { <Button on:tap={pause_todo_input}>Retry pause</Button> })
    } else {
        None
    };
    replace(
        ctx,
        "#todo-pause-control",
        html! {
            <Stack id="todo-pause-control">{notice}</Stack>
        },
    )
}

fn remaining(ctx: &Context, active: &Active) -> u64 {
    let duration = active.deadline.saturating_duration_since(ctx.clock().now());
    u64::try_from(duration.as_millis())
        .unwrap_or(u64::MAX)
        .div_ceil(1000)
}

fn timer(ctx: &Context, block: &Active) -> Html {
    let seconds = remaining(ctx, block);
    let clock = format!("{}:{:02}", seconds / 60, seconds % 60);
    let accessible = format!("{seconds} seconds remaining");
    html! { <SheetTitle id="todo-pause-timer" a11y_label={accessible}>{clock}</SheetTitle> }
}

fn pause_note(checking: bool) -> Html {
    let notice = if checking {
        "Waiting for the server to confirm the release."
    } else {
        "Todo updates still arrive."
    };
    html! { <Text id="todo-pause-note">{notice}</Text> }
}

fn show_modal(ctx: &Context, block: Option<&Active>) -> ActionResult {
    let modal = block.map(|block| {
        html! {
            <Modal id="todo-pause-modal" a11y_label="Input paused on this device">
                <Stack padding="16">
                    <SheetTitle id="todo-pause-title">Input paused on this device</SheetTitle>
                    {timer(ctx, block)}
                    {pause_note(block.checking)}
                </Stack>
            </Modal>
        }
    });
    replace(
        ctx,
        "#todo-countdown",
        html! { <Stack id="todo-countdown">{modal}</Stack> },
    )
}

pub(in crate::shared) async fn restore_view(ctx: &Context) -> ActionResult {
    let state = state(ctx)?;
    let state = state.lock().await;
    if state.session.is_some() {
        replace(ctx, "#todo-pause-button", button(true))?;
        show_controls(ctx, &state)?;
        show_modal(ctx, state.active.as_ref())?;
    }
    Ok(())
}

pub(super) async fn pause(ctx: &Context) -> ActionResult {
    let state = state(ctx)?;
    let request = {
        let mut state = state.lock().await;
        if state.active.is_some() || state.busy {
            return Ok(());
        }
        let mut session = state
            .session
            .clone()
            .ok_or_else(|| error("Countdown is not initialized"))?;
        let attempt = match session.pending.take() {
            Some(attempt) => attempt,
            None => Attempt {
                id: fresh_id(ctx)?,
                issued_at_ms: wall_ms(ctx),
            },
        };
        session.pending = Some(attempt.clone());
        write(ctx, &storage_key(&session), &session).await?;
        state.session = Some(session.clone());
        state.busy = true;
        state.retry = false;
        state.expired_request = false;
        show_controls(ctx, &state)?;
        Request {
            scope: session.scope,
            request: attempt.id,
            issued_at_ms: attempt.issued_at_ms,
        }
    };
    let result = ctx
        .trigger_structured("server:request_countdown", &request)
        .await;
    if let Err(cause) = result {
        let mut state = state.lock().await;
        state.busy = false;
        if cause.current_context().code == ErrorCode::InvalidInput {
            let mut session = state
                .session
                .clone()
                .ok_or_else(|| error("Missing countdown session"))?;
            session.pending = None;
            write(ctx, &storage_key(&session), &session).await?;
            state.session = Some(session);
            state.expired_request = true;
        }
        state.retry = state
            .session
            .as_ref()
            .is_some_and(|session| session.pending.is_some());
        show_controls(ctx, &state)?;
    }
    Ok(())
}

pub(super) async fn tick(ctx: &Context, input: Tick) -> ActionResult {
    let state = state(ctx)?;
    let query = {
        let mut state = state.lock().await;
        if state.session.is_none() {
            let session = load(ctx, input.session).await?;
            state.retry = session.pending.is_some();
            state.session = Some(session);
            replace(ctx, "#todo-pause-button", button(true))?;
            show_controls(ctx, &state)?;
        }
        if let Some(active) = &state.active {
            replace(ctx, "#todo-pause-timer", timer(ctx, active))?;
        }
        let needs_release = state
            .active
            .as_ref()
            .is_some_and(|active| remaining(ctx, active) <= 1);
        if state.initialized && !input.refresh && !needs_release {
            return Ok(());
        }
        Current {
            scope: state
                .session
                .as_ref()
                .ok_or_else(|| error("Missing countdown session"))?
                .scope
                .clone(),
            observed: state.active.as_ref().map(|active| active.block.id),
            await_release: needs_release,
        }
    };
    if let Err(cause) = ctx
        .trigger_structured("server:current_countdown", &query)
        .await
    {
        if query.observed.is_none() || cause.current_context().code != ErrorCode::ActionFailed {
            return Err(cause);
        }
        let mut state = state.lock().await;
        if let Some(active) = state
            .active
            .as_mut()
            .filter(|active| query.observed == Some(active.block.id))
        {
            if !active.checking {
                active.checking = true;
                replace(ctx, "#todo-pause-note", pause_note(true))?;
            }
        }
        return Ok(());
    }
    state.lock().await.initialized = true;
    Ok(())
}

pub(in crate::shared) async fn apply(ctx: &Context, update: Update) -> ActionResult {
    let state = state(ctx)?;
    let mut state = state.lock().await;
    let Some(mut session) = state.session.clone() else {
        return Ok(());
    };
    let reply_scope = match &update {
        Update::Active { scope, .. }
        | Update::Receipt { scope, .. }
        | Update::Released { scope, .. }
        | Update::Clear { scope, .. } => scope,
    };
    if reply_scope.session != session.scope.session {
        return Ok(());
    }
    match update {
        Update::Active {
            block,
            remaining_ms,
            ..
        } if state.released != Some(block.id) => {
            let deadline = ctx.clock().now() + Duration::from_millis(remaining_ms);
            if let Some(active) = &mut state.active {
                if active.block.id == block.id {
                    active.deadline = deadline;
                    if active.checking {
                        active.checking = false;
                        replace(ctx, "#todo-pause-note", pause_note(false))?;
                    }
                    return Ok(());
                }
                if active.block.expires_at_ms >= block.expires_at_ms {
                    return Ok(());
                }
            }
            session.pending = None;
            write(ctx, &storage_key(&session), &session).await?;
            state.session = Some(session);
            state.busy = false;
            state.retry = false;
            state.active = Some(Active {
                block,
                deadline,
                checking: false,
            });
            show_controls(ctx, &state)?;
            show_modal(ctx, state.active.as_ref())?;
        }
        Update::Receipt { request, .. }
            if session
                .pending
                .as_ref()
                .is_some_and(|attempt| attempt.id == request) =>
        {
            session.pending = None;
            write(ctx, &storage_key(&session), &session).await?;
            state.session = Some(session);
            state.busy = false;
            state.retry = false;
            show_controls(ctx, &state)?;
        }
        Update::Released { id, .. }
        | Update::Clear {
            observed: Some(id), ..
        } if state
            .active
            .as_ref()
            .is_some_and(|active| active.block.id == id) =>
        {
            state.released = Some(id);
            state.active = None;
            show_modal(ctx, None)?;
        }
        _ => {}
    }
    Ok(())
}

fn storage_key(session: &Session) -> String {
    #[cfg(all(feature = "web", target_family = "wasm"))]
    return key(Some(session.scope.session));
    #[cfg(not(all(feature = "web", target_family = "wasm")))]
    {
        let _ = session;
        key(None)
    }
}
