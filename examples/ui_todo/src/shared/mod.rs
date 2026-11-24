//! Shared todo views and actions for server and on-device stores.
pub use foundation_compact::uuid::UuidV4 as TodoId;
pub(crate) mod pending;
#[cfg(not(all(feature = "web", target_family = "wasm")))]
pub use pending::{install_pending_storage, PendingStorage};
use pending::{retry_todo, submit_delete, submit_todo};
pub use pending::{Pending, PendingChange};

use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use foundation_ui_runtime::action::{
    ActionError, ActionRegistry, ActionResult, Context, Interaction, RegistrationError,
};
use foundation_ui_runtime::state::{
    install_state_signer, HmacStateSigner, StateConfig, StateError,
};
use foundation_ui_runtime::view::{
    full_view, place, template_types::Html, PlacementKind, Target, ViewIds,
};
use foundation_ui_runtime::{action, html};
use foundation_ui_wire::ErrorCode;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Filter {
    #[default]
    All,
    Open,
    Done,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Todo {
    pub id: TodoId,
    pub text: String,
    pub done: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TodoApp {
    pub items: Vec<Todo>,
    pub filter: Filter,
    pub add_dialog: bool,
    pub delete_target: Option<TodoId>,
    #[serde(default)]
    pub draft: String,
    #[serde(default)]
    pub pending: std::collections::BTreeMap<String, Pending>,
}

#[derive(Default)]
pub struct TodoStore(Mutex<StoreState>);

#[derive(Clone, Default)]
struct StoreState {
    items: Vec<Todo>,
    filter: Filter,
    position: u64,
    query: String,
    pending: std::collections::BTreeMap<String, Pending>,
    search_rows: Option<Vec<Todo>>,
    correcting: Option<String>,
    add_dialog: bool,
    delete_target: Option<TodoId>,
    draft: String,
    deferred: std::collections::BTreeMap<u64, RecordChange>,
    removed: std::collections::BTreeMap<TodoId, u64>,
    retired_ids: std::collections::BTreeSet<TodoId>,
    creations: std::collections::BTreeMap<TodoId, CreationReceipt>,
}

#[derive(Clone)]
struct CreationReceipt {
    item: Todo,
    submitted_title: String,
    position: u64,
}

#[derive(Serialize, Deserialize)]
struct AddInput {
    id: TodoId,
    title: String,
}

impl StoreState {
    fn snapshot(&self) -> TodoApp {
        TodoApp {
            items: self.items.clone(),
            filter: self.filter.clone(),
            add_dialog: self.add_dialog,
            delete_target: self.delete_target,
            draft: self.draft.clone(),
            pending: self.pending.clone(),
        }
    }
}

impl TodoStore {
    /// Builds a store with its existing task identifiers.
    #[cfg(all(feature = "server", not(target_family = "wasm")))]
    pub(crate) fn with_items(items: Vec<Todo>) -> Self {
        Self(Mutex::new(StoreState {
            creations: items
                .iter()
                .cloned()
                .map(|item| {
                    (
                        item.id,
                        CreationReceipt {
                            submitted_title: item.text.clone(),
                            item,
                            position: 0,
                        },
                    )
                })
                .collect(),
            items,
            ..Default::default()
        }))
    }

    /// Capture the view and its published cursor while mutations hold the same lock.
    #[cfg(all(feature = "server", not(target_family = "wasm")))]
    pub(crate) fn published_snapshot(
        &self,
        feed: &foundation_wasm_ui::server::LiveChannel,
    ) -> (TodoApp, u64, Vec<TodoId>) {
        let state = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (
            state.snapshot(),
            feed.last_sequence(),
            state.retired_ids.iter().cloned().collect(),
        )
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
pub fn install_view_services(
    ctx: &Ctx,
    owner: foundation_ui_runtime::action::Origin,
) -> Result<(), StateError> {
    install_state_signer(
        ctx,
        Arc::new(HmacStateSigner::new(
            *b"demo-presentation-state-only-000",
            StateConfig::default(),
        )?),
    );
    ctx.store(ViewIds::for_origin(owner));
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

fn row(item: &Todo) -> Html {
    let id = item.id.to_string();
    let status = if item.done { "Reopen" } else { "Complete" };
    html! { <ChecklistRow checked={item.done}>
        <Text class="text-primary p-md ios-p-lg android-p-sm">{item.text.as_str()}</Text>
        <CheckmarkButton value={id.as_str()} on:tap={toggle_todo}>{status}</CheckmarkButton>
        <DestructiveButton value={id.as_str()} on:tap={open_delete_dialog}>Delete</DestructiveButton>
    </ChecklistRow> }
}

/// Render the shared Todo page in the receiving view.
///
/// # Errors
/// Returns a view or state serialization error.
pub fn app(ctx: &Context, state: &TodoApp) -> ActionResult {
    full_view(ctx, ctx.receiver(), &page_tree(state), None::<&()>)
}

fn list(state: &TodoApp, query: &str) -> Html {
    let rows = state
        .items
        .iter()
        .filter(|item| match state.filter {
            Filter::All => true,
            Filter::Open => !item.done,
            Filter::Done => item.done,
        })
        .map(
            |item| match state.pending.get(&format!("delete:{}", item.id)) {
                Some(pending) => pending_row(&item.text, "Deleting…", pending),
                None => row(item),
            },
        );
    let additions = state
        .pending
        .values()
        .filter_map(|pending| match &pending.change {
            PendingChange::Add { title, .. }
                if state.filter != Filter::Done
                    && (query.is_empty() || title.to_lowercase().contains(query)) =>
            {
                Some(pending_row(title, "Pending", pending))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    html! { <InsetList id="todo-list">{rows}{additions}</InsetList> }
}

fn pending_row(title: &str, status: &str, pending: &Pending) -> Html {
    let validation = pending.validation.as_deref();
    let retry = if validation.is_some() {
        match &pending.change {
            PendingChange::Add { id, .. } => Some(
                html! {<Button value={format!("correct-add:{id}")} on:tap={open_add_dialog}>Edit title</Button>},
            ),
            PendingChange::Delete { .. } => None,
        }
    } else if pending.retry {
        Some(match &pending.change {
            PendingChange::Add { id, .. } => {
                html! {<Button value={format!("add:{id}")} on:tap={retry_todo}>Try again</Button>}
            }
            PendingChange::Delete { id } => {
                html! {<Button value={format!("delete:{id}")} on:tap={retry_todo}>Try again</Button>}
            }
        })
    } else {
        None
    };
    let status = validation.unwrap_or(status);
    html! {<ChecklistRow class="animate-pulse opacity-70"><Text>{title}</Text><Text>{status}</Text>{retry}</ChecklistRow>}
}

fn filters(filter: &Filter) -> Html {
    html! { <SegmentedControl id="todo-filter" a11y_label="Filter todos">
        <Button value="all" checked={*filter == Filter::All} on:tap={filter_todos}>All</Button>
        <Button value="open" checked={*filter == Filter::Open} on:tap={filter_todos}>Open</Button>
        <Button value="done" checked={*filter == Filter::Done} on:tap={filter_todos}>Done</Button>
    </SegmentedControl> }
}

fn search_input(query: &str) -> Html {
    html! { <SearchInput id="todo-search" a11y_label="Search todos" placeholder="Search todos" value={query.to_owned()} on:change={search_todos}/> }
}

pub(crate) fn page_tree(state: &TodoApp) -> Html {
    let search = search_input("");
    let controls = filters(&state.filter);
    let rows = list(state, "");
    let footer = open_count(state.items.iter().filter(|item| !item.done).count());
    html! { <Stack>
        <NavigationTitle>Todos</NavigationTitle>
        <ToolbarButton on:tap={open_add_dialog}>Add</ToolbarButton>
        <Button on:tap={open_mathalley}>Play Mathalley</Button>
        {search}
        {controls}{rows}{footer}
        <Text id="notification"/><Stack id="dialog"/>
    </Stack> }
}

fn replace(ctx: &Context, name: &str, tree: Html) -> ActionResult {
    place(
        ctx,
        ctx.receiver(),
        PlacementKind::Swap,
        Target::from(name),
        None,
        Some(tree),
        None::<&()>,
    )
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
    full_view(ctx, ctx.receiver(), &starter_tree(count), None::<&()>)
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

fn dialog(ctx: &Context, state: &TodoApp) -> ActionResult {
    let cancel = html! {<CancelButton on:tap={close_dialog}>Cancel</CancelButton>};
    let confirmation = state.delete_target.map(|id| {
        html! {
            <ConfirmationDialog>
                <SheetTitle>Delete this todo?</SheetTitle>
                <DestructiveButton value={id.to_string()} on:tap={submit_delete}>Confirm</DestructiveButton>
                {cancel.clone()}
            </ConfirmationDialog>
        }
    });
    let tree = html! { <Stack id="dialog">
        <Either condition={state.add_dialog}>
            <Left>{confirmation}</Left>
            <Right><Modal a11y_label="New todo"><BottomSheet>
                <SheetTitle>New todo</SheetTitle>
                <TextInput placeholder="Todo title" a11y_label="Todo title" value={state.draft.as_str()} on:change={edit_draft} on:submit={submit_todo}/>
                {cancel.clone()}
                <SubmitButton>Add</SubmitButton>
            </BottomSheet></Modal></Right>
        </Either>
    </Stack> };
    replace(ctx, "#dialog", tree)
}

#[action(host)]
async fn open_mathalley(ctx: &Context, _event: Interaction<()>) -> ActionResult {
    let signal = crate::mathalley::route::open_command().map_err(|cause| error(&cause.to_string()))?;
    ctx.receiver().data_in_view(signal);
    Ok(())
}

#[action(host)]
async fn open_add_dialog(ctx: &Context, event: Interaction<()>) -> ActionResult {
    let store = todo_store(ctx)?;
    let mut cache = store
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    cache.correcting = None;
    if let Some(id) = event.input_value.strip_prefix("correct-add:") {
        let key = format!("add:{id}");
        let Some(Pending {
            change: PendingChange::Add { title, .. },
            validation: Some(_),
            ..
        }) = cache.pending.get(&key)
        else {
            return Err(error("This submission is no longer pending"));
        };
        cache.draft = title.clone();
        cache.correcting = Some(key);
    }
    cache.add_dialog = true;
    cache.delete_target = None;
    dialog(ctx, &cache.snapshot())
}
#[action(host)]
async fn open_delete_dialog(ctx: &Context, event: Interaction<()>) -> ActionResult {
    let id = todo_id(&event.input_value)?;
    let store = todo_store(ctx)?;
    let mut cache = store
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    cache.correcting = None;
    cache.add_dialog = false;
    cache.delete_target = Some(id);
    dialog(ctx, &cache.snapshot())
}
#[action(host)]
async fn close_dialog(ctx: &Context, _event: Interaction<()>) -> ActionResult {
    let store = todo_store(ctx)?;
    let mut cache = store
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    cache.correcting = None;
    cache.add_dialog = false;
    cache.delete_target = None;
    cache.draft.clear();
    dialog(ctx, &cache.snapshot())
}
#[action(host)]
async fn filter_todos(ctx: &Context, event: Interaction<()>) -> ActionResult {
    let filter = match event.input_value.as_str() {
        "all" => Filter::All,
        "open" => Filter::Open,
        "done" => Filter::Done,
        _ => return Err(error("Unknown filter")),
    };
    let store = todo_store(ctx)?;
    let mut guard = store
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.filter = filter;
    replace(ctx, "#todo-filter", filters(&guard.filter))?;
    guard.select_search();
    guard.render_rows(ctx)
}

#[action(host)]
async fn edit_draft(ctx: &Context, event: Interaction<()>) -> ActionResult {
    let store = todo_store(ctx)?;
    store
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .draft = event.input_value;
    Ok(())
}

#[action(host)]
async fn search_todos(ctx: &Context, event: Interaction<()>) -> ActionResult {
    let store = todo_store(ctx)?;
    let mut cache = store
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    cache.query = event.input_value;
    cache.select_search();
    cache.render_rows(ctx)
}

impl StoreState {
    fn select_search(&mut self) {
        let query = self.query.trim().to_lowercase();
        self.search_rows = (!query.is_empty()).then(|| {
            self.items
                .iter()
                .filter(|item| item.text.to_lowercase().contains(&query))
                .cloned()
                .collect()
        });
    }
    fn render_rows(&self, ctx: &Context) -> ActionResult {
        let mut state = self.snapshot();
        if let Some(rows) = &self.search_rows {
            state.items.clone_from(rows);
        }
        let query = self.query.trim().to_lowercase();
        replace(ctx, "#todo-list", list(&state, &query))
    }
}

fn todo_store(ctx: &Context) -> Result<Arc<TodoStore>, ErrorTrace<ActionError>> {
    ctx.get::<TodoStore>()
        .ok_or_else(|| error("Missing todo store"))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum RecordChange {
    Upsert(Todo),
    Remove(TodoId),
}

#[derive(Serialize, Deserialize)]
pub(crate) enum TodoDelivery {
    Snapshot {
        position: u64,
        items: Vec<Todo>,
        #[serde(default)]
        retired_ids: Vec<TodoId>,
    },
    Retired {
        id: TodoId,
    },
    Committed {
        position: u64,
        change: RecordChange,
    },
    Deleted {
        id: TodoId,
    },
}

impl TodoDelivery {
    pub(crate) fn send(&self, ctx: &Context) -> ActionResult {
        let bytes =
            rmp_serde::to_vec_named(&("ewe.todo.v1", self)).map_err(|e| error(&e.to_string()))?;
        ctx.receiver().data_in_view(bytes);
        Ok(())
    }
}

impl StoreState {
    fn apply_change(&mut self, change: &RecordChange) {
        match change {
            RecordChange::Upsert(item) => {
                if let Some(row) = self.items.iter_mut().find(|row| row.id == item.id) {
                    row.clone_from(item);
                } else {
                    self.items.push(item.clone());
                }
            }
            RecordChange::Remove(id) => {
                if let Some(index) = self.items.iter().position(|row| row.id == *id) {
                    self.items.remove(index);
                }
                self.retired_ids.insert(*id);
            }
        }
    }

    fn ingest(&mut self, delivery: TodoDelivery) -> bool {
        let before = self.position;
        let snapshot = match delivery {
            TodoDelivery::Retired { id } => return self.retired_ids.insert(id),
            TodoDelivery::Snapshot {
                position,
                items,
                retired_ids,
            } if position >= self.position => {
                self.retired_ids.extend(retired_ids);
                self.items = items;
                self.position = position;
                self.deferred.retain(|seq, _| *seq > position);
                true
            }
            TodoDelivery::Committed { position, change } if position > self.position => {
                self.deferred.entry(position).or_insert(change);
                false
            }
            _ => return false,
        };
        while let Some(next) = self.position.checked_add(1) {
            let Some(change) = self.deferred.remove(&next) else {
                break;
            };
            if let RecordChange::Remove(id) = &change {
                self.removed.insert(*id, next);
            }
            self.apply_change(&change);
            self.position = next;
        }
        snapshot || self.position != before
    }
}

#[action(host)]
async fn apply_todo_delivery(ctx: &Context, event: Interaction<(), Vec<u8>>) -> ActionResult {
    // The view can also receive data for other applications.
    if !event.input_value.starts_with(b"\x92\xabewe.todo.v1") {
        return Ok(());
    }
    let (_, delivery): (String, TodoDelivery) =
        rmp_serde::from_slice(&event.input_value).map_err(|e| error(&e.to_string()))?;
    let store = todo_store(ctx)?;
    let snapshot = matches!(delivery, TodoDelivery::Snapshot { .. });
    let changed = {
        let mut state = store
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &delivery {
            TodoDelivery::Deleted { id } => {
                if state.delete_target == Some(*id) {
                    state.delete_target = None;
                    dialog(ctx, &state.snapshot())?;
                }
                return Ok(());
            }
            _ => {}
        }
        state.ingest(delivery)
    };
    let reconciled = pending::reconcile(ctx, snapshot).await?;
    if reconciled.added {
        replace(
            ctx,
            "#notification",
            html! { <Text id="notification">Todo added</Text> },
        )?;
    }
    if changed || reconciled.changed {
        let state = store
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.search_rows.is_none() || snapshot || reconciled.changed {
            state.render_rows(ctx)?;
        }
        replace(
            ctx,
            "#open-count",
            open_count(state.items.iter().filter(|r| !r.done).count()),
        )?;
        if snapshot {
            if !state.query.is_empty() {
                replace(ctx, "#todo-search", search_input(&state.query))?;
            }
            replace(ctx, "#todo-filter", filters(&state.filter))?;
            if state.add_dialog || state.delete_target.is_some() {
                dialog(ctx, &state.snapshot())?;
            }
        }
    }
    Ok(())
}

fn mutate(
    ctx: &Context,
    plan: impl FnOnce(&StoreState) -> Result<RecordChange, ErrorTrace<ActionError>>,
) -> ActionResult {
    let store = todo_store(ctx)?;
    let mut state = store
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let change = plan(&state)?;
    if let RecordChange::Remove(id) = &change {
        if let Some(&position) = state.removed.get(id) {
            return TodoDelivery::Committed { position, change }.send(ctx);
        }
    }
    commit(ctx, &mut state, change)?.send(ctx)
}

fn commit(
    ctx: &Context,
    state: &mut StoreState,
    change: RecordChange,
) -> Result<TodoDelivery, ErrorTrace<ActionError>> {
    let position = state
        .position
        .checked_add(1)
        .ok_or_else(|| error("Todo stream position exhausted"))?;
    #[cfg(all(feature = "server", not(target_family = "wasm")))]
    let position = match ctx.get::<crate::server::TodoFeed>() {
        Some(feed) => feed
            .publish(ctx.inner(), &change)
            .map_err(|e| error(&e.to_string()))?,
        None => position,
    };
    state.apply_change(&change);
    if let RecordChange::Remove(id) = &change {
        state.removed.insert(*id, position);
    }
    state.position = position;
    Ok(TodoDelivery::Committed { position, change })
}

#[action(server)]
async fn add_todo(ctx: &Context, event: Interaction<(), AddInput>) -> ActionResult {
    let input = event.input_value;
    let title = input.title.trim();
    if title.is_empty() {
        return Err(ErrorTrace::new(ActionError::new(
            ErrorCode::InvalidInput,
            "Enter a todo title",
        )));
    }
    let store = todo_store(ctx)?;
    let mut state = store
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(receipt) = state.creations.get(&input.id) {
        if receipt.submitted_title != input.title {
            return Err(ErrorTrace::new(ActionError::new(
                ErrorCode::InvalidInput,
                "The creation identifier already belongs to a different submission",
            )));
        }
        let (position, change) = match state.removed.get(&receipt.item.id) {
            Some(position) => {
                TodoDelivery::Retired { id: input.id }.send(ctx)?;
                (*position, RecordChange::Remove(receipt.item.id))
            }
            None => (receipt.position, RecordChange::Upsert(receipt.item.clone())),
        };
        return TodoDelivery::Committed { position, change }.send(ctx);
    }
    let item = Todo {
        id: input.id,
        text: title.to_owned(),
        done: false,
    };
    let delivery = commit(ctx, &mut state, RecordChange::Upsert(item.clone()))?;
    let position = state.position;
    state.creations.insert(
        input.id,
        CreationReceipt {
            item,
            position,
            submitted_title: input.title,
        },
    );
    delivery.send(ctx)?;
    Ok(())
}

#[action(server)]
async fn toggle_todo(ctx: &Context, event: Interaction<()>) -> ActionResult {
    let id = todo_id(&event.input_value)?;
    mutate(ctx, |state| {
        let mut item = state
            .items
            .iter()
            .find(|row| row.id == id)
            .cloned()
            .ok_or_else(|| error("Todo does not exist"))?;
        item.done = !item.done;
        Ok(RecordChange::Upsert(item))
    })
}

#[action(server)]
async fn confirm_delete(ctx: &Context, event: Interaction<(), TodoId>) -> ActionResult {
    let id = event.input_value;
    mutate(ctx, |state| {
        if !state.removed.contains_key(&id) && !state.items.iter().any(|row| row.id == id) {
            return Err(error("Todo does not exist"));
        }
        Ok(RecordChange::Remove(id))
    })?;
    TodoDelivery::Deleted { id }.send(ctx)
}

fn todo_id(input: &str) -> Result<TodoId, ErrorTrace<ActionError>> {
    input
        .parse()
        .map_err(|reason| ErrorTrace::new(ActionError::new(ErrorCode::InvalidInput, reason)))
}

#[action(host, mobile)]
async fn initialize_todos(ctx: &Context, _event: Interaction<()>) -> ActionResult {
    pending::restore(ctx).await?;
    app(ctx, &todo_store(ctx)?.snapshot())
}

/// Install the local display cache.
pub fn install_cache(ctx: &Ctx) {
    ctx.store(TodoStore::default());
    #[cfg(all(feature = "web", target_family = "wasm"))]
    pending::install_browser(ctx);
    #[cfg(not(all(feature = "web", target_family = "wasm")))]
    install_pending_storage(ctx, Arc::new(pending::MemoryStorage::default()));
}

fn action_registry(
    owner: foundation_ui_runtime::action::Origin,
) -> Result<ActionRegistry, RegistrationError> {
    let mut registry = ActionRegistry::new();
    registry.register_for(owner, initialize_todos)?;
    registry.register_for(owner, open_mathalley)?;
    registry.register_for(owner, open_add_dialog)?;
    registry.register_for(owner, open_delete_dialog)?;
    registry.register_for(owner, close_dialog)?;
    registry.register_for(owner, filter_todos)?;
    registry.register_for(owner, search_todos)?;
    registry.register_for(owner, edit_draft)?;
    registry.register_for(owner, apply_todo_delivery)?;
    registry.register_for(owner, submit_todo)?;
    registry.register_for(owner, submit_delete)?;
    registry.register_for(owner, retry_todo)?;
    registry.register_for(owner, refresh_count)?;
    registry.register_for(owner, add_todo)?;
    registry.register_for(owner, toggle_todo)?;
    registry.register_for_with_input(owner, confirm_delete, |input| {
        use foundation_ui_runtime::action::RawInput;
        match input {
            RawInput::Text(text) => todo_id(&text),
            structured => structured.decode::<TodoId>(),
        }
    })?;
    Ok(registry)
}

/// Build this application's server endpoint.
///
/// # Errors
/// Reports conflicting declarations.
pub fn server_registry() -> Result<ActionRegistry, RegistrationError> {
    action_registry(foundation_ui_runtime::action::Origin::Server)
}

/// Build this application's display endpoint.
///
/// # Errors
/// Reports conflicting declarations.
pub fn host_registry() -> Result<ActionRegistry, RegistrationError> {
    action_registry(foundation_ui_runtime::action::Origin::Host)
}
