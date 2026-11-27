//! Serialize business changes before their durable write and live publication.
#[cfg(all(feature = "server", not(target_family = "wasm")))]
use super::Arc;
use super::{
    error, todo_store, ActionError, ActionResult, AddInput, Context, CreationReceipt, Ctx,
    ErrorCode, ErrorTrace, RecordChange, StoreState, Todo, TodoDelivery, TodoId, TodoStore,
    ViewData,
};

pub(super) enum Command {
    Add(AddInput),
    Toggle(TodoId),
    Remove(TodoId),
}

enum Plan {
    Replay {
        delivery: TodoDelivery,
        retired: Option<TodoId>,
    },
    Write {
        change: RecordChange,
        receipt: Option<CreationReceipt>,
    },
}

impl Command {
    fn plan(self, state: &StoreState, position: u64) -> Result<Plan, ErrorTrace<ActionError>> {
        match self {
            Self::Add(input) => {
                let title = input.title.trim();
                if title.is_empty() {
                    return Err(ErrorTrace::new(ActionError::new(
                        ErrorCode::InvalidInput,
                        "Enter a todo title",
                    )));
                }
                if let Some(receipt) = state.creations.get(&input.id) {
                    if receipt.submitted_title != input.title {
                        return Err(ErrorTrace::new(ActionError::new(
                            ErrorCode::InvalidInput,
                            "The creation identifier already belongs to a different submission",
                        )));
                    }
                    let id = input.id;
                    return Ok(match state.removed.get(&id) {
                        Some(&position) => Plan::Replay {
                            delivery: TodoDelivery::Committed {
                                position,
                                change: RecordChange::Remove(id),
                            },
                            retired: Some(id),
                        },
                        None => Plan::Replay {
                            delivery: TodoDelivery::Committed {
                                position: receipt.position,
                                change: RecordChange::Upsert(receipt.item.clone()),
                            },
                            retired: None,
                        },
                    });
                }
                let item = Todo {
                    id: input.id,
                    text: title.to_owned(),
                    done: false,
                };
                Ok(Plan::Write {
                    change: RecordChange::Upsert(item.clone()),
                    receipt: Some(CreationReceipt {
                        item,
                        submitted_title: input.title,
                        position,
                    }),
                })
            }
            Self::Toggle(id) => {
                let mut item = state
                    .items
                    .iter()
                    .find(|row| row.id == id)
                    .cloned()
                    .ok_or_else(|| error("Todo does not exist"))?;
                item.done = !item.done;
                Ok(Plan::Write {
                    change: RecordChange::Upsert(item),
                    receipt: None,
                })
            }
            Self::Remove(id) => {
                let change = RecordChange::Remove(id);
                if let Some(&position) = state.removed.get(&id) {
                    return Ok(Plan::Replay {
                        delivery: TodoDelivery::Committed { position, change },
                        retired: None,
                    });
                }
                if !state.items.iter().any(|row| row.id == id) {
                    return Err(error("Todo does not exist"));
                }
                Ok(Plan::Write {
                    change,
                    receipt: None,
                })
            }
        }
    }
}

#[cfg(all(feature = "server", not(target_family = "wasm")))]
pub(super) struct Writer(async_channel::Sender<Request>);

#[cfg(all(feature = "server", not(target_family = "wasm")))]
struct Request {
    store: Arc<TodoStore>,
    host: Ctx,
    command: Command,
    reply: async_channel::Sender<CommitResult>,
}

type CommitResult = Result<(TodoDelivery, Option<TodoId>), ErrorTrace<ActionError>>;

#[cfg(all(feature = "server", not(target_family = "wasm")))]
impl Writer {
    pub(super) fn start() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        use foundation_core::valtron::{from_future, send};
        let (sender, requests) = async_channel::unbounded::<Request>();
        send(from_future(async move {
            while let Ok(request) = requests.recv().await {
                let Request {
                    store,
                    host,
                    command,
                    reply,
                } = request;
                let outcome = commit(&store, &host, command).await;
                drop(store);
                drop(host);
                // A disconnected client already dropped its only reply receiver.
                drop(reply.send(outcome).await);
            }
        }))
        .map_err(|cause| cause.to_string())?;
        Ok(Self(sender))
    }

    async fn submit(&self, store: Arc<TodoStore>, host: Ctx, command: Command) -> CommitResult {
        let (reply, response) = async_channel::bounded(1);
        self.0
            .send(Request {
                store,
                host,
                command,
                reply,
            })
            .await
            .map_err(|_| error("Todo writer stopped before accepting a request"))?;
        response
            .recv()
            .await
            .map_err(|_| error("Todo writer stopped before responding"))?
    }
}

pub(super) async fn apply(ctx: &Context, command: Command) -> ActionResult {
    let store = todo_store(ctx)?;
    let host = ctx.inner().clone();
    #[cfg(all(feature = "server", not(target_family = "wasm")))]
    let result = if let Some(writer) = &store.1.writer {
        writer.submit(store.clone(), host, command).await
    } else {
        commit(&store, &host, command).await
    };
    #[cfg(not(all(feature = "server", not(target_family = "wasm"))))]
    let result = commit(&store, &host, command).await;
    let (delivery, retired) = result?;
    if let Some(id) = retired {
        TodoDelivery::Retired { id }.send(ctx)?;
    }
    delivery.send(ctx)
}

async fn commit(store: &TodoStore, host: &Ctx, command: Command) -> CommitResult {
    #[cfg(not(all(feature = "server", not(target_family = "wasm"))))]
    let _ = host;
    let _writer = store.1.gate.lock().await;
    let (plan, previous) = {
        let state = store
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (
            command.plan(&state, state.position.saturating_add(1))?,
            state.position,
        )
    };
    let (change, receipt) = match plan {
        Plan::Replay { delivery, retired } => return Ok((delivery, retired)),
        Plan::Write { change, receipt } => (change, receipt),
    };
    let position = previous
        .checked_add(1)
        .ok_or_else(|| error("Todo stream position exhausted"))?;
    #[cfg(all(feature = "server", not(target_family = "wasm")))]
    let publication = host
        .get::<crate::server::TodoFeed>()
        .map(|feed| feed.reserve(host, &change))
        .transpose()
        .map_err(|cause| error(&cause.to_string()))?;
    #[cfg(all(feature = "server", not(target_family = "wasm")))]
    {
        if publication
            .as_ref()
            .is_some_and(|frame| frame.position() != position)
        {
            return Err(error("Todo stream position differs from its store"));
        }
        if let Some(database) = &store.1.database {
            let original = if let Some(receipt) = &receipt {
                receipt.clone()
            } else {
                let id = match &change {
                    RecordChange::Upsert(row) => row.id,
                    RecordChange::Remove(id) => *id,
                };
                store
                    .0
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .creations
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| error("Missing Todo creation receipt"))?
            };
            database
                .save(original, &change, position)
                .await
                .map_err(|cause| error(&cause.to_string()))?;
        }
    }
    let mut state = store
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(receipt) = receipt {
        state.creations.insert(receipt.item.id, receipt);
    }
    state.apply_change(&change);
    if let RecordChange::Remove(id) = &change {
        state.removed.insert(*id, position);
    }
    state.position = position;
    #[cfg(all(feature = "server", not(target_family = "wasm")))]
    if let Some(frame) = publication {
        frame.publish();
    }
    Ok((TodoDelivery::Committed { position, change }, None))
}
