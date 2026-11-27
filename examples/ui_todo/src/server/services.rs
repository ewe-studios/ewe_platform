//! Configure the host and publish the list and count after a successful change.
use super::Error;
use crate::{TodoDelivery, TodoStore};
use foundation_context::Ctx;
use foundation_ui_http::ActionRequestPolicy;
use foundation_ui_runtime::action::ViewData;
use foundation_ui_runtime::action::{prepare_messages, ActionError, Context};
use foundation_ui_wire::{writer::WriterConfig, ActionRequest, ErrorCode};
use foundation_wasm_ui::server::{BacklogConfig, LiveChannel};
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct TodoFeed(pub(super) LiveChannel);

impl TodoFeed {
    pub(crate) fn reserve(
        &self,
        ctx: &Ctx,
        change: &crate::RecordChange,
    ) -> Result<foundation_wasm_ui::server::LivePublication, Error> {
        self.0.sender().reserve(|position| {
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
    if store.position() != web.last_sequence() {
        return Err("The Todo store position differs from the live channel position".into());
    }
    crate::install_view_services(ctx, foundation_ui_runtime::action::Origin::Server)?;
    crate::shared::countdown::install(ctx);
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

/// Read Todo records from the request context through the generated service.
pub struct Todos;
impl crate::todo_rpc::Todos for Todos {
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
            .get(id)
            .await
            .map_err(|cause| ConnectError::internal(cause.to_string()))?
            .ok_or_else(|| ConnectError::not_found("Todo not found"))?;
        Ok(Response::new(crate::todo_rpc::Todo {
            id: todo.id.to_string(),
            title: todo.text,
            ..Default::default()
        }))
    }
}
