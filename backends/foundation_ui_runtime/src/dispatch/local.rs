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
