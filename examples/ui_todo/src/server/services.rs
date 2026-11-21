//! Configure the host and publish counts after a successful change.
use super::{Error, COUNT_NODE};
use crate::{open_count, TodoStore, PAGE_ROOT};
use foundation_browser::test::{BacklogConfig, LiveChannel};
use foundation_context::Ctx;
use foundation_ui_http::ActionRequestPolicy;
use foundation_ui_runtime::{
    action::{prepare_messages, ActionError, Context},
    dispatch::RenderTarget,
    html_swap,
    view::render,
};
use foundation_ui_wire::{writer::WriterConfig, ActionRequest, ErrorCode, MountSurface};
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct TodoChannels {
    pub(super) web: LiveChannel,
    pub(super) native: LiveChannel,
}

impl TodoChannels {
    pub(crate) fn publish(&self, ctx: &Ctx, count: usize) -> Result<(), Error> {
        let web = Context::for_render(ctx.clone());
        html_swap! {"#count-value",&web,web.receiver(),<span id="count-value">{count}</span>}?;
        self.web
            .sender()
            .push_messages(prepare_messages(ctx, web.into_receiver().drain())?)?;
        let native = Context::for_action(ctx.clone(), MountSurface::Native, Some(COUNT_NODE));
        render(&native, native.receiver(), open_count(count), None::<&()>)?;
        self.native
            .sender()
            .push_messages(prepare_messages(ctx, native.into_receiver().drain())?)?;
        Ok(())
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

pub(super) fn install(ctx: &Ctx, web: LiveChannel) -> Result<TodoChannels, Error> {
    if web.stream_id() != 1 {
        return Err("The todo page listens to stream 1".into());
    }
    crate::install_view_services(ctx, 100, (1 << 31) - 1)?;
    ctx.store(RenderTarget(PAGE_ROOT));
    ctx.store(TodoStore::default());
    ctx.store(Arc::new(BusinessActions) as Arc<dyn ActionRequestPolicy>);
    let channels = TodoChannels {
        web,
        native: channel(2),
    };
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
        let store = ctx
            .ctx
            .get::<TodoStore>()
            .ok_or_else(|| ConnectError::internal("Missing todo store"))?;
        let todo = store
            .snapshot()
            .items
            .into_iter()
            .find(|todo| todo.id == request.msg.id)
            .ok_or_else(|| ConnectError::not_found("Todo not found"))?;
        Ok(Response::new(crate::todo_rpc::Todo {
            id: todo.id,
            title: todo.text,
            ..Default::default()
        }))
    }
}
