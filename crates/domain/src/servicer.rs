// Module implementing DomainService default implementations and related tests

use crate::domains::{self, NamedEvent, NamedRequest};
use channels::{
    channels::{ReceiveChannel, SendChannel, SendOnlyChannel},
    executor,
};

struct ChannelGroup<E>(ReceiveChannel<E>, SendChannel<E>);

pub struct DServicer<E: Send + 'static, R: Send + 'static, P: Default> {
    execution_service: executor::ExecutionService<NamedEvent<E>>,
    executor: executor::Executor<NamedEvent<E>>,
    incoming_requests: ChannelGroup<NamedRequest<R>>,
    outgoing_requests: ChannelGroup<NamedRequest<R>>,
    events: ChannelGroup<NamedRequest<E>>,
    platform: P,
}

impl<E: Send + 'static, R: Send + 'static, P: Default> domains::DomainServicer
    for DServicer<E, R, P>
{
    type Events = domains::NamedEvent<E>;

    type Requests = domains::NamedRequest<R>;

    type Platform = P;

    fn shell(
        &self,
    ) -> &dyn crate::domains::DomainShell<
        Platform = Self::Platform,
        Events = Self::Events,
        Requests = Self::Requests,
    > {
        todo!()
    }

    fn serve(
        &mut self,
        d: &dyn crate::domains::Domain<
            Shell = Self,
            Events = Self::Events,
            Requests = Self::Requests,
            Platform = Self::Platform,
        >,
    ) {
        todo!()
    }
}
