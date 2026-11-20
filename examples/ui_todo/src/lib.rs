//! Shared todo views and actions. The server owns the list.
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use foundation_ui_runtime::action::{
    ActionError, ActionRegistry, ActionResult, Context, Interaction, RegistrationError,
};
use foundation_ui_runtime::state::{
    install_state_signer, HmacStateSigner, StateConfig, StateError,
};
use foundation_ui_runtime::view::{render, template_types::Html, ViewIds};
use foundation_ui_runtime::{action, html};
use foundation_ui_wire::{ErrorCode, MountSurface};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[cfg(all(feature = "server", not(target_family = "wasm")))]
pub mod server;

/// Stable roots created by the page shell.
pub const PAGE_ROOT: u32 = 40;
pub const DIALOG_ROOT: u32 = 41;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Filter {
    #[default]
    All,
    Open,
    Done,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Todo {
    pub id: u64,
    pub text: String,
    pub done: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TodoApp {
    pub items: Vec<Todo>,
    pub filter: Filter,
    pub add_dialog: bool,
    pub delete_target: Option<u64>,
}

#[derive(Default)]
pub struct TodoStore(Mutex<(u64, Vec<Todo>)>);

impl TodoStore {
    #[must_use]
    pub fn snapshot(&self) -> TodoApp {
        TodoApp {
            items: self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .1
                .clone(),
            ..TodoApp::default()
        }
    }
}

fn error(message: &str) -> ErrorTrace<ActionError> {
    ErrorTrace::new(ActionError::new(ErrorCode::ActionFailed, message))
}

/// Sets up presentation snapshots. The server rejects incoming business requests with attachments.
///
/// # Errors
/// Returns the validation error for the presentation signer.
pub fn install_view_services(ctx: &Ctx, first: u32, last: u32) -> Result<(), StateError> {
    install_state_signer(
        ctx,
        Arc::new(HmacStateSigner::new(
            *b"demo-presentation-state-only-000",
            StateConfig::default(),
        )?),
    );
    ctx.store(ViewIds::range(first, last));
    Ok(())
}

fn row(item: &Todo) -> Html {
    let id = item.id.to_string();
    html! { <Stack axis="horizontal">
        <Text>{item.text.as_str()}</Text>
        <Button value={id.as_str()} on:tap={toggle_todo}>{if item.done {"Reopen"}else{"Complete"}}</Button>
        <Button value={id.as_str()} on:tap={open_delete_dialog}>Delete</Button>
    </Stack> }
}

fn page(ctx: &Context, state: &TodoApp, web: bool) -> ActionResult {
    let out = ctx.receiver();
    let title = if web {
        html! {<Button on:tap="host:scroll_to_top">Todos</Button>}
    } else {
        html! {<Text>Todos</Text>}
    };
    let rows = state
        .items
        .iter()
        .filter(|item| match state.filter {
            Filter::Done => item.done,
            Filter::Open => !item.done,
            Filter::All => true,
        })
        .map(row);
    let count = state.items.iter().filter(|item| !item.done).count();
    let count_view = if web {
        html! {<div id="open-count"><span id="count-value">{count}</span><mount-stream data-channel-id="1" api="/live" protocol="frame" target="#live-sink"/></div>}
    } else {
        html! {<Text>{count}</Text>}
    };
    html! { ctx, out, Some(state), <Stack>
        {title}{count_view}
        <Button primal::state on:tap={open_add_dialog}>Add</Button>
        <Stack axis="horizontal">
            <Button value="all" primal::state on:tap={filter_todos}>All</Button>
            <Button value="open" primal::state on:tap={filter_todos}>Open</Button>
            <Button value="done" primal::state on:tap={filter_todos}>Done</Button>
        </Stack>
        <List><div data-ewe-behaviour="scroll_lock"/>{rows}</List>
    </Stack> };
    Ok(())
}

/// Renders the shared todo page.
///
/// # Errors
/// Returns a view or state serialization error.
pub fn app(ctx: &Context, state: &TodoApp) -> ActionResult {
    page(ctx, state, false)
}
/// Renders the todo page with its web header and live count.
///
/// # Errors
/// Returns a view or state serialization error.
pub fn web_app(ctx: &Context, state: &TodoApp) -> ActionResult {
    page(ctx, state, true)
}

fn dialog(ctx: &Context, state: &TodoApp) -> ActionResult {
    let shell = Context::for_action(ctx.inner().clone(), ctx.surface(), Some(DIALOG_ROOT));
    let tree = if let Some(id) = state.delete_target {
        html! { <Overlay><div data-ewe-behaviour="dialog"/><Text>Delete this todo?</Text>
        <Button value={id} on:tap={confirm_delete}>Confirm</Button>
        <Button on:tap={close_dialog}>Cancel</Button></Overlay> }
    } else if state.add_dialog && ctx.surface() == MountSurface::Web {
        html! { <Overlay><div data-ewe-behaviour="dialog"/>
        <form on:submit={add_todo}><input autofocus="autofocus" name="title" aria-label="Todo title" />
        <button type="submit">Confirm</button></form>
        <Button on:tap={close_dialog}>Cancel</Button></Overlay> }
    } else if state.add_dialog {
        html! { <Overlay><div data-ewe-behaviour="dialog"/><TextInput placeholder="Todo title" on:submit={add_todo}/>
        <Button on:tap={close_dialog}>Cancel</Button></Overlay> }
    } else {
        html! {<Stack/>}
    };
    render(&shell, ctx.receiver(), tree, None::<&()>)?;
    Ok(())
}

#[action(host)]
async fn open_add_dialog(ctx: &Context, event: Interaction<TodoApp>) -> ActionResult {
    let mut state = event.state;
    state.add_dialog = true;
    dialog(ctx, &state)
}
#[action(host)]
async fn open_delete_dialog(ctx: &Context, event: Interaction<()>) -> ActionResult {
    let id: u64 = event
        .input_value
        .parse()
        .map_err(|_| error("Invalid todo identifier"))?;
    dialog(
        ctx,
        &TodoApp {
            delete_target: Some(id),
            ..TodoApp::default()
        },
    )
}
#[action(host)]
async fn close_dialog(ctx: &Context, _event: Interaction<()>) -> ActionResult {
    dialog(ctx, &TodoApp::default())
}
#[action(host)]
async fn filter_todos(ctx: &Context, event: Interaction<TodoApp>) -> ActionResult {
    let mut state = event.state;
    state.filter = match event.input_value.as_str() {
        "all" => Filter::All,
        "open" => Filter::Open,
        "done" => Filter::Done,
        _ => return Err(error("Unknown filter")),
    };
    page(ctx, &state, ctx.surface() == MountSurface::Web)
}

fn mutate(
    ctx: &Context,
    change: impl FnOnce(&mut u64, &mut Vec<Todo>) -> ActionResult,
) -> ActionResult {
    let store = ctx
        .inner()
        .get::<TodoStore>()
        .ok_or_else(|| error("Missing todo store"))?;
    let state = {
        let mut guard = store
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (last, items) = &mut *guard;
        change(last, items)?;
        TodoApp {
            items: items.clone(),
            ..TodoApp::default()
        }
    };
    page(ctx, &state, ctx.surface() == MountSurface::Web)?;
    dialog(ctx, &state)
}

#[action(server)]
async fn add_todo(ctx: &Context, event: Interaction<()>) -> ActionResult {
    let title = event.input_value.trim();
    if title.is_empty() {
        return Err(error("Enter a todo title"));
    }
    mutate(ctx, |last, items| {
        *last = last
            .checked_add(1)
            .ok_or_else(|| error("No more todo identifiers"))?;
        items.push(Todo {
            id: *last,
            text: title.to_owned(),
            done: false,
        });
        Ok(())
    })?;
    // MessagePack fixstr for the page notification.
    ctx.receiver().data(PAGE_ROOT, b"\xabhost:notify".to_vec());
    Ok(())
}
#[action(server)]
async fn toggle_todo(ctx: &Context, event: Interaction<()>) -> ActionResult {
    let id: u64 = event
        .input_value
        .parse()
        .map_err(|_| error("Invalid todo identifier"))?;
    mutate(ctx, |_, items| {
        let item = items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or_else(|| error("Todo does not exist"))?;
        item.done = !item.done;
        Ok(())
    })
}
#[action(server)]
async fn confirm_delete(ctx: &Context, event: Interaction<()>) -> ActionResult {
    let id: u64 = event
        .input_value
        .parse()
        .map_err(|_| error("Invalid todo identifier"))?;
    mutate(ctx, |_, items| {
        let index = items
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| error("already deleted"))?;
        items.remove(index);
        Ok(())
    })
}

/// Builds the server action registry.
///
/// # Errors
/// Returns an invalid or duplicate action declaration.
pub fn server_registry() -> Result<ActionRegistry, RegistrationError> {
    let mut actions = ActionRegistry::new();
    actions.register(add_todo)?;
    actions.register(toggle_todo)?;
    actions.register(confirm_delete)?;
    Ok(actions)
}
/// Builds the browser or native host action registry.
///
/// # Errors
/// Returns an invalid or duplicate action declaration.
pub fn host_registry() -> Result<ActionRegistry, RegistrationError> {
    let mut actions = ActionRegistry::new();
    actions.register(open_add_dialog)?;
    actions.register(open_delete_dialog)?;
    actions.register(close_dialog)?;
    actions.register(filter_todos)?;
    actions.register(notify)?;
    Ok(actions)
}

/// Mobile hosts render the same notification through a shared action.
#[action(host, mobile)]
async fn notify(ctx: &Context, _event: Interaction<()>) -> ActionResult {
    let out = ctx.receiver();
    foundation_ui_runtime::html_add! {PAGE_ROOT,ctx,out,<Text>Todo added</Text>}
}
