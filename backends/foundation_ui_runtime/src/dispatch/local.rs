use foundation_context::Ctx;

use super::{Connection, Dispatcher};
use crate::action::{ActionTransport, TransportFuture};

/// Sends requests to a dispatcher in the same host and returns its reply.
pub struct LocalDispatcher {
    dispatcher: Dispatcher,
    ctx: Ctx,
    connection: Connection,
}

impl LocalDispatcher {
    /// Claims the request now. The returned task keeps its dispatch context until completion.
    pub fn submit_owned(self: std::sync::Arc<Self>, frame: &[u8]) -> TransportFuture<'static> {
        let admission = self.dispatcher.admit(&self.connection, frame);
        Box::pin(async move {
            Ok(Some(
                self.dispatcher
                    .finish(&self.ctx, &self.connection, admission)
                    .await,
            ))
        })
    }

    #[must_use]
    pub fn new(dispatcher: Dispatcher, ctx: Ctx, connection: Connection) -> Self {
        Self {
            dispatcher,
            ctx,
            connection,
        }
    }
}

impl ActionTransport for LocalDispatcher {
    fn send(&self, frame: Vec<u8>) -> TransportFuture<'_> {
        let reply = self.dispatcher.submit(&self.ctx, &self.connection, &frame);
        Box::pin(async move { Ok(Some(reply.await)) })
    }
}
