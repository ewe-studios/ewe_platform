// Module implementing DomainService default implementations and related tests

use std::sync;

use channels::{executor, mspc};

use crate::domains::{self, NamedEvent, NamedRequest};

pub struct DShell<E: Send + 'static, R: Send + 'static, P: Clone> {
    platform: P,
    executor: executor::Executor<NamedEvent<'static, E>>,
    incoming_requests: mspc::ChannelGroup<NamedRequest<'static, R>>,
    outgoing_requests: mspc::ChannelGroup<NamedRequest<'static, R>>,
    incoming_events: mspc::ChannelGroup<NamedRequest<'static, E>>,
    outgoing_events: mspc::ChannelGroup<NamedEvent<'static, E>>,
    // registery:
}

impl<E: Send + 'static, R: Send + 'static, P: Clone> domains::DomainShell for DShell<E, R, P> {
    type Events = E;

    type Requests = R;

    type Platform = P;

    fn platform(&self) -> Self::Platform
    where
        Self: Sized,
    {
        self.platform.clone()
    }

    fn respond(&self, _event: NamedEvent<Self::Events>) {
        todo!()
    }

    fn do_requests(
        &self,
        _req: NamedRequest<Self::Requests>,
    ) -> mspc::ReceiveChannel<NamedEvent<Self::Events>> {
        todo!()
    }

    fn send_requests(
        &self,
        _req: NamedRequest<Self::Requests>,
    ) -> mspc::ReceiveChannel<NamedEvent<Self::Events>> {
        todo!()
    }

    fn schedule(
        &self,
        _receiver: mspc::ReceiveChannel<Self::Events>,
        _fut: impl futures::prelude::Future<Output = ()> + 'static + Send,
    ) where
        Self: Sized,
    {
        todo!()
    }

    fn spawn(&self, _fut: impl futures::prelude::Future<Output = ()> + 'static + Send)
    where
        Self: Sized,
    {
        todo!()
    }

    fn requests(&self) -> mspc::ReceiveChannel<NamedRequest<sync::Arc<Self::Requests>>> {
        todo!()
    }

    fn listen(&self) -> mspc::ReceiveChannel<sync::Arc<Self::Events>> {
        todo!()
    }
}

pub struct DServicer<E: Send + 'static, R: Send + 'static, P: Clone> {
    shell: sync::Arc<DShell<E, R, P>>,
    execution_service: executor::ExecutionService<NamedEvent<'static, E>>,
}

impl<E: Send + 'static, R: Send + 'static, P: Clone> DServicer<E, R, P> {
    pub fn new() -> Self {
        todo!()
    }

    pub fn process(&self) {
        todo!()
    }
}

impl<E: Send + 'static, R: Send + 'static, P: Clone> domains::DomainServicer
    for DServicer<E, R, P>
{
    type Events = domains::NamedEvent<'static, E>;

    type Requests = domains::NamedRequest<'static, R>;

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
        _d: &dyn crate::domains::Domain<
            Shell = Self,
            Events = Self::Events,
            Requests = Self::Requests,
            Platform = Self::Platform,
        >,
    ) {
        todo!()
    }
}
