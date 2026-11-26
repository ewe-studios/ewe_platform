//! Register native handlers on the existing route tree.
use std::{future::Future, marker::PhantomData, sync::Arc};

use crate::{
    native::serve::Serve,
    shared::{
        router::IntoHandler,
        serve::{ConnectionResult, ServeOutcome},
    },
    Ctx, RawStream, SharedByteBufferStream, SimpleIncomingRequest,
};

type Connection = SharedByteBufferStream<RawStream>;

/// Identifies an existing Serve handler during type inference.
pub struct Existing;
/// Identifies a function that returns a connection result.
pub struct Immediate;
/// Identifies a function that returns a future.
pub struct Deferred;

impl<H: Serve> IntoHandler<Existing> for H {
    type Stored = Arc<dyn Serve>;
    fn into_handler(self) -> Self::Stored {
        Arc::new(self)
    }
}

struct Function<F, Kind>(F, PhantomData<fn() -> Kind>);

impl<F> Serve for Function<F, Immediate>
where
    F: Fn(Ctx, SimpleIncomingRequest, Connection) -> ConnectionResult + Send + Sync + 'static,
{
    fn serve(&self, ctx: Ctx, req: SimpleIncomingRequest, conn: Connection) -> ConnectionResult {
        (self.0)(ctx, req, conn)
    }
}

impl<F, Fut> Serve for Function<F, Deferred>
where
    F: Fn(Ctx, SimpleIncomingRequest, Connection) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ServeOutcome> + Send + 'static,
{
    fn serve(&self, ctx: Ctx, req: SimpleIncomingRequest, conn: Connection) -> ConnectionResult {
        ConnectionResult::Async(Box::pin((self.0)(ctx, req, conn)))
    }
}

impl<F> IntoHandler<Immediate> for F
where
    F: Fn(Ctx, SimpleIncomingRequest, Connection) -> ConnectionResult + Send + Sync + 'static,
{
    type Stored = Arc<dyn Serve>;
    fn into_handler(self) -> Self::Stored {
        Arc::new(Function::<_, Immediate>(self, PhantomData))
    }
}

impl<F, Fut> IntoHandler<Deferred> for F
where
    F: Fn(Ctx, SimpleIncomingRequest, Connection) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ServeOutcome> + Send + 'static,
{
    type Stored = Arc<dyn Serve>;
    fn into_handler(self) -> Self::Stored {
        Arc::new(Function::<_, Deferred>(self, PhantomData))
    }
}
