//! Shared todo views and actions for server and on-device stores.
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
pub struct TodoStore(Mutex<StoreState>);

#[derive(Default)]
struct StoreState {
    last_id: u64,
    items: Vec<Todo>,
    filter: Filter,
}

impl StoreState {
    fn snapshot(&self) -> TodoApp {
        TodoApp {
            items: self.items.clone(),
            filter: self.filter.clone(),
            ..TodoApp::default()
        }
    }
}

struct OnDevice;

impl TodoStore {
    #[must_use]
    pub fn snapshot(&self) -> TodoApp {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .snapshot()
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

fn row(item: &Todo, web: bool, local: bool) -> Html {
    let id = item.id.to_string();
    let status = if item.done { "Reopen" } else { "Complete" };
    if web {
        return html! { <Stack axis="horizontal">
            <Text>{item.text.as_str()}</Text>
            <Button value={id.as_str()} on:tap={toggle_todo}>{status}</Button>
            <Button value={id.as_str()} on:tap={open_delete_dialog}>Delete</Button>
        </Stack> };
    }
    let toggle = if local {
        html! {<Button presentation="checkmark-action" value={id.as_str()} on:tap={toggle_local_todo}>{status}</Button>}
    } else {
        html! {<Button presentation="checkmark-action" value={id.as_str()} on:tap={toggle_todo}>{status}</Button>}
    };
    html! { <Stack axis="horizontal" presentation="checklist-row" checked={item.done}>
        <Text>{item.text.as_str()}</Text>
        {toggle}
        <Button presentation="destructive-action" value={id.as_str()} on:tap={open_delete_dialog}>Delete</Button>
    </Stack> }
}

fn page(ctx: &Context, state: &TodoApp, web: bool) -> ActionResult {
    let out = ctx.receiver();
    let local = ctx.inner().get::<OnDevice>().is_some();
    let rows = state
        .items
        .iter()
        .filter(|item| match state.filter {
            Filter::Done => item.done,
            Filter::Open => !item.done,
            Filter::All => true,
        })
        .map(|item| row(item, web, local));
    let count = state.items.iter().filter(|item| !item.done).count();
    if web {
        html! { ctx, out, Some(state), <Stack>
            <Button on:tap="host:scroll_to_top">Todos</Button>
            <div id="open-count"><span id="count-value">{count}</span><mount-stream data-channel-id="1" api="/live" protocol="frame" target="#live-sink"/></div>
            <Button primal::state on:tap={open_add_dialog}>Add</Button>
            <Stack axis="horizontal">
                <Button value="all" primal::state on:tap={filter_todos}>All</Button>
                <Button value="open" primal::state on:tap={filter_todos}>Open</Button>
                <Button value="done" primal::state on:tap={filter_todos}>Done</Button>
            </Stack>
            <List><div data-ewe-behaviour="scroll_lock"/>{rows}</List>
        </Stack> };
    } else {
        let count_view = if local {
            open_count(count)
        } else {
            Html::default()
        };
        html! { ctx, out, Some(state), <Stack>
            <Text presentation="navigation-title">Todos</Text>
            <Button presentation="toolbar-action" primal::state on:tap={open_add_dialog}>Add</Button>
            <Stack axis="horizontal" presentation="segmented-control" a11y_label="Filter todos">
                <Button value="all" checked={state.filter == Filter::All} primal::state on:tap={filter_todos}>All</Button>
                <Button value="open" checked={state.filter == Filter::Open} primal::state on:tap={filter_todos}>Open</Button>
                <Button value="done" checked={state.filter == Filter::Done} primal::state on:tap={filter_todos}>Done</Button>
            </Stack>
            <List presentation="inset-list">{rows}</List>
            {count_view}
        </Stack> };
    }
    Ok(())
}

fn open_count(count: usize) -> Html {
    let unit = if count == 1 { "todo" } else { "todos" };
    html! {<Text presentation="list-footer">{format!("{count} open {unit}")}</Text>}
}

/// Builds a screen whose only action runs on the server.
///
/// # Errors
/// Returns a view or serialization error.
pub fn starter_view(ctx: &Context, count: usize) -> ActionResult {
    html! {ctx, ctx.receiver(), <Stack><Text>Open todos</Text><Text>{count}</Text>
    <Button on:tap={refresh_count}>Refresh</Button></Stack>};
    Ok(())
}

#[action(server)]
async fn refresh_count(ctx: &Context, _event: Interaction<()>) -> ActionResult {
    let store = ctx
        .inner()
        .get::<TodoStore>()
        .ok_or_else(|| error("Missing todo store"))?;
    starter_view(
        ctx,
        store
            .snapshot()
            .items
            .iter()
            .filter(|item| !item.done)
            .count(),
    )
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
    let local = ctx.inner().get::<OnDevice>().is_some();
    let web = ctx.surface() == MountSurface::Web;
    let tree = if let Some(id) = state.delete_target.filter(|_| web) {
        html! { <Overlay><div data-ewe-behaviour="dialog"/><div data-ewe-behaviour="dismiss"/><div data-ewe-behaviour="transition"/><Stack><Text>Delete this todo?</Text>
        <Button value={id} on:tap={confirm_delete}>Confirm</Button>
        <Button on:tap={close_dialog}>Cancel</Button></Stack></Overlay> }
    } else if let Some(id) = state.delete_target {
        let confirm = if local {
            html! {<Button presentation="destructive-action" value={id} on:tap={confirm_local_delete}>Confirm</Button>}
        } else {
            html! {<Button presentation="destructive-action" value={id} on:tap={confirm_delete}>Confirm</Button>}
        };
        html! { <Overlay presentation="confirmation-dialog">
            <div data-ewe-behaviour="dialog"/><div data-ewe-behaviour="dismiss"/><div data-ewe-behaviour="transition"/>
            <Text presentation="sheet-title">Delete this todo?</Text>
            {confirm}
            <Button presentation="cancel-action" on:tap={close_dialog}>Cancel</Button>
        </Overlay> }
    } else if state.add_dialog && web {
        html! { <Overlay><div data-ewe-behaviour="dialog"/><div data-ewe-behaviour="dismiss"/><div data-ewe-behaviour="transition"/>
        <Stack><form on:submit={add_todo}><input autofocus="autofocus" name="title" aria-label="Todo title" />
        <button type="submit">Confirm</button></form>
        <Button on:tap={close_dialog}>Cancel</Button></Stack></Overlay> }
    } else if state.add_dialog {
        let input = if local {
            html! {<TextInput placeholder="Todo title" a11y_label="Todo title" on:submit={add_local_todo}/>}
        } else {
            html! {<TextInput placeholder="Todo title" a11y_label="Todo title" on:submit={add_todo}/>}
        };
        html! { <Overlay presentation="form-sheet">
            <div data-ewe-behaviour="dialog"/><div data-ewe-behaviour="dismiss"/><div data-ewe-behaviour="transition"/>
            <Text presentation="sheet-title">New todo</Text>
            {input}
            <Button presentation="submit-action">Add</Button>
            <Button presentation="cancel-action" on:tap={close_dialog}>Cancel</Button>
        </Overlay> }
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
    let filter = match event.input_value.as_str() {
        "all" => Filter::All,
        "open" => Filter::Open,
        "done" => Filter::Done,
        _ => return Err(error("Unknown filter")),
    };
    if ctx.inner().get::<OnDevice>().is_some() {
        let store = todo_store(ctx)?;
        let mut guard = store
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.filter = filter;
        return page(ctx, &guard.snapshot(), false);
    }
    state.filter = filter;
    page(ctx, &state, ctx.surface() == MountSurface::Web)
}

fn todo_store(ctx: &Context) -> Result<Arc<TodoStore>, ErrorTrace<ActionError>> {
    ctx.inner()
        .get::<TodoStore>()
        .ok_or_else(|| error("Missing todo store"))
}

fn mutate(
    ctx: &Context,
    change: impl FnOnce(&mut u64, &mut Vec<Todo>) -> ActionResult,
) -> ActionResult {
    let store = todo_store(ctx)?;
    let mut guard = store
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let StoreState { last_id, items, .. } = &mut *guard;
    change(last_id, items)?;
    let state = guard.snapshot();
    page(ctx, &state, ctx.surface() == MountSurface::Web)?;
    dialog(ctx, &state)
}

#[action(server)]
async fn add_todo(ctx: &Context, event: Interaction<()>) -> ActionResult {
    add(ctx, &event.input_value)
}

fn add(ctx: &Context, input: &str) -> ActionResult {
    let title = input.trim();
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
    if ctx.inner().get::<OnDevice>().is_none() {
        // MessagePack fixstr for the page notification.
        ctx.receiver().data(PAGE_ROOT, b"\xabhost:notify".to_vec());
    }
    Ok(())
}
#[action(server)]
async fn toggle_todo(ctx: &Context, event: Interaction<()>) -> ActionResult {
    toggle(ctx, &event.input_value)
}

fn toggle(ctx: &Context, input: &str) -> ActionResult {
    let id = todo_id(input)?;
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
    delete(ctx, &event.input_value)
}

fn delete(ctx: &Context, input: &str) -> ActionResult {
    let id = todo_id(input)?;
    mutate(ctx, |_, items| {
        let index = items
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| error("already deleted"))?;
        items.remove(index);
        Ok(())
    })
}

fn todo_id(input: &str) -> Result<u64, ErrorTrace<ActionError>> {
    input.parse().map_err(|_| error("Invalid todo identifier"))
}

#[action(host, mobile)]
async fn add_local_todo(ctx: &Context, event: Interaction<()>) -> ActionResult {
    add(ctx, &event.input_value)
}

#[action(host, mobile)]
async fn toggle_local_todo(ctx: &Context, event: Interaction<()>) -> ActionResult {
    toggle(ctx, &event.input_value)
}

#[action(host, mobile)]
async fn confirm_local_delete(ctx: &Context, event: Interaction<()>) -> ActionResult {
    delete(ctx, &event.input_value)
}

#[action(host, mobile)]
async fn initialize_todos(ctx: &Context, _event: Interaction<()>) -> ActionResult {
    use foundation_ui_wire::{CreateNodeT, NodeKind, OperationT, UiOperationT};

    let out = ctx.receiver();
    for root in [PAGE_ROOT, DIALOG_ROOT] {
        let mut create = CreateNodeT::default();
        create.node_id = root;
        create.kind = NodeKind::STACK;
        let mut op = OperationT::default();
        op.ui = UiOperationT::CreateNode(Box::new(create));
        out.render_op(op);
        out.render_op(foundation_ui_wire::ops::append_child(1, root));
    }
    app(ctx, &todo_store(ctx)?.snapshot())
}

/// Installs the on-device store and returns its action registry.
///
/// # Errors
/// Returns an invalid or duplicate action declaration.
pub fn local_registry(ctx: &Ctx) -> Result<ActionRegistry, RegistrationError> {
    let mut actions = host_registry()?;
    actions.register(initialize_todos)?;
    actions.register(add_local_todo)?;
    actions.register(toggle_local_todo)?;
    actions.register(confirm_local_delete)?;
    ctx.store(TodoStore::default());
    ctx.store(OnDevice);
    Ok(actions)
}

/// Builds the server action registry.
///
/// # Errors
/// Returns an invalid or duplicate action declaration.
pub fn server_registry() -> Result<ActionRegistry, RegistrationError> {
    let mut actions = ActionRegistry::new();
    actions.register(refresh_count)?;
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
