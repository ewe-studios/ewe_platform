//! Configure the host and publish the list and count after a successful change.
use super::Error;
use crate::{TodoDelivery, TodoStore};
use foundation_context::Ctx;
use foundation_ui_http::ActionRequestPolicy;
use foundation_ui_runtime::action::{prepare_messages, ActionError, Context};
use foundation_ui_wire::{writer::WriterConfig, ActionRequest, ErrorCode};
use foundation_wasm_ui::server::{BacklogConfig, LiveChannel};
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct TodoFeed(pub(super) LiveChannel);

impl TodoFeed {
    pub(crate) fn publish(&self, ctx: &Ctx, change: &crate::RecordChange) -> Result<u64, Error> {
        self.0.sender().push_with(|position| {
            let view = Context::for_render(ctx.clone());
            TodoDelivery::Committed {
                position,
                change: change.clone(),
            }
            .send(&view)?;
            Ok(prepare_messages(ctx, view.into_receiver().drain())?)
        })
    }
}

struct BusinessActions;
impl ActionRequestPolicy for BusinessActions {
    fn check(&self, _: &Ctx, action: ActionRequest<'_>) -> Result<(), ActionError> {
        if action.state().is_some() {
            return Err(ActionError::new(
                ErrorCode::StateRejected,
                "Business actions reject presentation state",
            ));
        }
        Ok(())
    }
}

pub(super) fn install(ctx: &Ctx, web: LiveChannel, store: TodoStore) -> Result<TodoFeed, Error> {
    if web.stream_id() != 1 {
        return Err("The todo page listens to stream 1".into());
    }
    crate::install_view_services(ctx, foundation_ui_runtime::action::Origin::Server)?;
    ctx.store(store);
    ctx.store(Arc::new(BusinessActions) as Arc<dyn ActionRequestPolicy>);
    let channels = TodoFeed(web);
    let feed = channels.0.clone();
    ctx.store(foundation_ui_runtime::dispatch::SubscriptionReplay::new(
        move |_, _| vec![Arc::new(feed.clone())],
    ));
    ctx.store(channels.clone());
    Ok(channels)
}

pub(super) fn channel(id: u64) -> LiveChannel {
    LiveChannel::new(id, BacklogConfig::default(), WriterConfig::default())
}

pub(super) struct Todos;
impl crate::todo_rpc::Todos for Todos {
    #[expect(
        clippy::unused_async_trait_impl,
        reason = "The generated service has an asynchronous contract."
    )]
    async fn get_todo(
        &self,
        ctx: foundation_connectrpc::RpcCtx,
        request: foundation_connectrpc::Request<crate::todo_rpc::GetTodoRequest>,
    ) -> foundation_connectrpc::ConnectResult<foundation_connectrpc::Response<crate::todo_rpc::Todo>>
    {
        use foundation_connectrpc::{ConnectError, Response};
        let id = request
            .msg
            .id
            .parse::<crate::TodoId>()
            .map_err(ConnectError::invalid_argument)?;
        let store = ctx
            .ctx
            .get::<TodoStore>()
            .ok_or_else(|| ConnectError::internal("Missing todo store"))?;
        let todo = store
            .snapshot()
            .items
            .into_iter()
            .find(|todo| todo.id == id)
            .ok_or_else(|| ConnectError::not_found("Todo not found"))?;
        Ok(Response::new(crate::todo_rpc::Todo {
            id: todo.id.to_string(),
            title: todo.text,
            ..Default::default()
        }))
    }
}
