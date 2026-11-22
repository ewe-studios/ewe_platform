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
use foundation_ui_wire::ErrorCode;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

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

#[derive(Clone, Default)]
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
    /// Builds a store with one known task for the standalone RPC example.
    #[cfg(all(feature = "server", not(target_family = "wasm")))]
    pub(crate) fn with_item(item: Todo) -> Self {
        Self(Mutex::new(StoreState {
            last_id: item.id,
            items: vec![item],
            ..Default::default()
        }))
    }

    /// Capture the view and its published cursor while mutations hold the same lock.
    #[cfg(all(feature = "server", not(target_family = "wasm")))]
    pub(crate) fn published_snapshot(
        &self,
        feed: &foundation_wasm_ui::server::LiveChannel,
    ) -> (TodoApp, u64) {
        let state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (state.snapshot(), feed.last_sequence())
    }

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

/// Defines the optional application theme for the three renderers.
#[must_use]
pub fn todo_theme() -> foundation_theme::GeneratedTheme {
    foundation_theme::Theme::new()
        .color("primary", "#155f59", Some("#9ae4d8"))
        .spacing("md", "8px")
        .spacing("lg", "12px")
        .spacing("sm", "4px")
        .build()
}

fn row(item: &Todo, local: bool) -> Html {
    let id = item.id.to_string();
    let status = if item.done { "Reopen" } else { "Complete" };
    html! { <ChecklistRow checked={item.done}>
        <Text class="text-primary p-md ios-p-lg android-p-sm">{item.text.as_str()}</Text>
        <Either condition={local}>
            <Left><CheckmarkButton value={id.as_str()} on:tap={toggle_todo}>{status}</CheckmarkButton></Left>
            <Right><CheckmarkButton value={id.as_str()} on:tap={toggle_local_todo}>{status}</CheckmarkButton></Right>
        </Either>
        <DestructiveButton value={id.as_str()} on:tap={open_delete_dialog}>Delete</DestructiveButton>
    </ChecklistRow> }
}

fn page(ctx: &Context, state: &TodoApp) -> ActionResult {
    let (tree, state) = page_tree(state, ctx.get::<OnDevice>().is_some());
    render(ctx, ctx.receiver(), tree, Some(state))?;
    Ok(())
}

pub(crate) fn page_tree(state: &TodoApp, local: bool) -> (Html, &TodoApp) {
    let rows = state
        .items
        .iter()
        .filter(|item| match state.filter {
            Filter::Done => item.done,
            Filter::Open => !item.done,
            Filter::All => true,
        })
        .map(|item| row(item, local));
    let count = state.items.iter().filter(|item| !item.done).count();
    let count_view = open_count(count);
    html! { state = state, <Stack>
        <NavigationTitle>Todos</NavigationTitle>
        <ToolbarButton primal::state on:tap={open_add_dialog}>Add</ToolbarButton>
        <SegmentedControl a11y_label="Filter todos">
            <Button value="all" checked={state.filter == Filter::All} primal::state on:tap={filter_todos}>All</Button>
            <Button value="open" checked={state.filter == Filter::Open} primal::state on:tap={filter_todos}>Open</Button>
            <Button value="done" checked={state.filter == Filter::Done} primal::state on:tap={filter_todos}>Done</Button>
        </SegmentedControl>
        <InsetList>{rows}</InsetList>
        {count_view}
    </Stack> }
}

pub(crate) fn open_count(count: usize) -> Html {
    html! {<ListFooter id="open-count">{open_count_label(count)}</ListFooter>}
}

pub(crate) fn open_count_label(count: usize) -> String {
    let unit = if count == 1 { "todo" } else { "todos" };
    format!("{count} open {unit}")
}

/// Builds a screen whose only action runs on the server.
///
/// # Errors
/// Returns a view or serialization error.
pub fn starter_view(ctx: &Context, count: usize) -> ActionResult {
    render(ctx, ctx.receiver(), starter_tree(count), None::<&()>)?;
    Ok(())
}

pub(crate) fn starter_tree(count: usize) -> Html {
    html! { <Stack><Text>Open todos</Text><Text id="open-count">{count}</Text>
    <Button on:tap={refresh_count}>Refresh</Button></Stack> }
}

#[action(server)]
async fn refresh_count(ctx: &Context, _event: Interaction<()>) -> ActionResult {
    let store = ctx
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
    page(ctx, state)
}
/// Renders the shared page for existing browser callers.
///
/// # Errors
/// Returns a view or state serialization error.
pub fn web_app(ctx: &Context, state: &TodoApp) -> ActionResult {
    page(ctx, state)
}

fn dialog(ctx: &Context, state: &TodoApp) -> ActionResult {
    let shell = Context::for_action(ctx.inner().clone(), ctx.surface(), Some(DIALOG_ROOT));
    let local = ctx.get::<OnDevice>().is_some();
    let tree = if let Some(id) = state.delete_target {
        let confirm = if local {
            html! {<DestructiveButton value={id} on:tap={confirm_local_delete}>Confirm</DestructiveButton>}
        } else {
            html! {<DestructiveButton value={id} on:tap={confirm_delete}>Confirm</DestructiveButton>}
        };
        html! { <ConfirmationDialog>
            <SheetTitle>Delete this todo?</SheetTitle>
            {confirm}
            <CancelButton on:tap={close_dialog}>Cancel</CancelButton>
        </ConfirmationDialog> }
    } else if state.add_dialog {
        let input = if local {
            html! {<TextInput placeholder="Todo title" a11y_label="Todo title" on:submit={add_local_todo}/>}
        } else {
            html! {<TextInput placeholder="Todo title" a11y_label="Todo title" on:submit={add_todo}/>}
        };
        html! { <Modal a11y_label="New todo"><BottomSheet>
            <SheetTitle>New todo</SheetTitle>
            {input}
            <CancelButton on:tap={close_dialog}>Cancel</CancelButton>
            <SubmitButton>Add</SubmitButton>
        </BottomSheet></Modal> }
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
    if ctx.get::<OnDevice>().is_some() {
        let store = todo_store(ctx)?;
        let mut guard = store
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.filter = filter;
        return page(ctx, &guard.snapshot());
    }
    state.filter = filter;
    page(ctx, &state)
}

fn todo_store(ctx: &Context) -> Result<Arc<TodoStore>, ErrorTrace<ActionError>> {
    ctx.get::<TodoStore>()
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
    let mut next = guard.clone();
    change(&mut next.last_id, &mut next.items)?;
    let state = next.snapshot();
    page(ctx, &state)?;
    dialog(ctx, &state)?;
    #[cfg(all(feature = "server", not(target_family = "wasm")))]
    if let Some(channels) = ctx.get::<crate::server::TodoFeed>() {
        channels
            .publish(
                ctx.inner(),
                state.items.iter().filter(|todo| !todo.done).count(),
            )
            .map_err(|failure| error(&failure.to_string()))?;
    }
    *guard = next;
    Ok(())
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
    if ctx.get::<OnDevice>().is_none() {
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
