// Module implementing DomainService default implementations and related tests

use channels::{broadcast, executor, mspc};

use std::sync;

use crate::{
    domains::{self, NamedEvent, NamedRequest},
    pending_chan,
};

pub struct DShell<E: Send + Clone + 'static, R: Send + Clone + 'static, P: Clone> {
    shell_platform: P,
    executor: sync::Arc<executor::Executor<NamedEvent<E>>>,
    event_broadcast: broadcast::Broadcast<NamedEvent<E>>,
    request_broadcast: broadcast::Broadcast<NamedRequest<R>>,
    incoming_request_sender: mspc::SendChannel<NamedRequest<R>>,
    response_registry: pending_chan::PendingChannelsRegistry<NamedEvent<E>>,
}

impl<E: Send + Clone + 'static, R: Send + Clone + 'static, P: Clone> Clone for DShell<E, R, P> {
    fn clone(&self) -> Self {
        Self {
            executor: self.executor.clone(),
            shell_platform: self.shell_platform.clone(),
            request_broadcast: self.request_broadcast.clone(),
            event_broadcast: self.event_broadcast.clone(),
            response_registry: self.response_registry.clone(),
            incoming_request_sender: self.incoming_request_sender.clone(),
        }
    }
}

impl<E: Send + Clone + 'static, R: Send + Clone + 'static, P: Clone> domains::MasterShell
    for DShell<E, R, P>
{
    fn send_requests(
        &mut self,
        req: NamedRequest<Self::Requests>,
    ) -> domains::DomainOpsResult<mspc::ReceiveChannel<NamedEvent<Self::Events>>, Self::Requests>
    {
        let resolution_channel = self.response_registry.register(req.id());
        self.request_broadcast.broadcast(req.clone());
        Ok(resolution_channel.1.clone())
    }

    fn send_events(
        &mut self,
        event: NamedEvent<Self::Events>,
    ) -> domains::DomainOpsResult<(), Self::Events> {
        _ = self.response_registry.register(event.id());
        self.event_broadcast.broadcast(event.clone());
        Ok(())
    }
}

impl<E: Send + Clone + 'static, R: Send + Clone + 'static, P: Clone> domains::DomainShell
    for DShell<E, R, P>
{
    type Events = E;

    type Requests = R;

    type Platform = P;

    fn platform(&self) -> Self::Platform
    where
        Self: Sized,
    {
        self.shell_platform.clone()
    }

    fn respond(
        &mut self,
        event: NamedEvent<Self::Events>,
    ) -> domains::DomainOpsResult<(), Self::Events> {
        match self.response_registry.resolve(event.id(), event.clone()) {
            Ok(_) => Ok(()),
            Err(pending_chan::PendingChannelError::NotFound(_)) => {
                Err(domains::DomainOpsErrors::NoMatchingRequestForEvent(event))
            }
            Err(pending_chan::PendingChannelError::ClosedSender(_, _)) => {
                Err(domains::DomainOpsErrors::UnableToDeliverEvents(event))
            }
        }
    }

    fn do_requests(
        &mut self,
        req: NamedRequest<Self::Requests>,
    ) -> domains::DomainOpsResult<mspc::ReceiveChannel<NamedEvent<Self::Events>>, Self::Requests>
    {
        // create resolution channel group, send the RetreiveChannel to the user.
        let resolution_channel = self.response_registry.register(req.id());
        match self.incoming_request_sender.try_send(req.clone()) {
            Ok(_) => Ok(resolution_channel.1.clone()),
            Err(_) => Err(domains::DomainOpsErrors::UnableToSendRequest(req)),
        }
    }

    fn schedule<Fut>(
        &self,
        receiver: mspc::ReceiveChannel<NamedEvent<Self::Events>>,
        receiver_fn: impl FnOnce(mspc::Result<NamedEvent<Self::Events>>) -> Fut + 'static + Send,
    ) -> domains::DomainResult<()>
    where
        Fut: futures::prelude::future::Future<Output = ()> + Send,
        Self: Sized,
    {
        match self.executor.schedule(receiver, receiver_fn) {
            Ok(_) => Ok(()),
            Err(_) => Err(domains::DomainErrors::FailedScheduling),
        }
    }

    fn spawn(
        &self,
        fut: impl futures::prelude::Future<Output = ()> + 'static + Send,
    ) -> domains::DomainResult<()>
    where
        Self: Sized,
    {
        match self.executor.spawn(fut) {
            Ok(_) => Ok(()),
            Err(_) => Err(domains::DomainErrors::FailedScheduling),
        }
    }

    fn requests(
        &mut self,
    ) -> domains::DomainResult<mspc::ReceiveChannel<sync::Arc<NamedRequest<Self::Requests>>>> {
        Ok(self.request_broadcast.subscribe())
    }

    fn listen(
        &mut self,
    ) -> domains::DomainResult<mspc::ReceiveChannel<sync::Arc<NamedEvent<Self::Events>>>> {
        Ok(self.event_broadcast.subscribe())
    }
}

pub struct DServicer<E: Send + Clone + 'static, R: Send + Clone + 'static, P: Clone> {
    domain_shell: DShell<E, R, P>,
    execution_service: executor::ExecutionService<NamedEvent<E>>,
    incoming_request_receiver: mspc::ReceiveChannel<NamedRequest<R>>,
    response_registry: pending_chan::PendingChannelsRegistry<NamedEvent<E>>,
}

const DEFAULT_SUBSCRIBER_START_CAPACITY: usize = 10;

pub fn create_servicer<E: Send + Clone + 'static, R: Send + Clone + 'static, P: Clone>(
    shell_platform: P,
) -> DServicer<E, R, P> {
    let (incoming_request_sender, incoming_request_receiver) = mspc::create();
    let (execution_service, executor) = executor::create();
    let event_broadcast = broadcast::create::<NamedEvent<E>>(DEFAULT_SUBSCRIBER_START_CAPACITY);
    let request_broadcast = broadcast::create::<NamedRequest<R>>(DEFAULT_SUBSCRIBER_START_CAPACITY);
    let response_registry = pending_chan::PendingChannelsRegistry::new();

    let executor_arc = sync::Arc::new(executor);

    DServicer {
        domain_shell: DShell {
            shell_platform,
            incoming_request_sender,
            executor: executor_arc,
            request_broadcast: request_broadcast.clone(),
            event_broadcast: event_broadcast.clone(),
            response_registry: response_registry.clone(),
        },
        execution_service: execution_service,
        incoming_request_receiver,
        response_registry,
    }
}

impl<E: Send + Clone + 'static, R: Send + Clone + 'static, P: Clone> DServicer<E, R, P> {
    fn process_incoming_request(
        &mut self,
        domain: impl domains::Domain<Events = E, Requests = R, Platform = P>,
    ) {
        while let Ok(request) = self.incoming_request_receiver.try_receive() {
            if let Some(response_grp) = self.response_registry.retrieve(request.id()) {
                domain.handle_request(request, response_grp.0.clone(), self.domain_shell.clone());
                continue;
            }
            assert!(false, "failed to find response registry for {}", request);
        }
    }
}

impl<E: Send + Clone + 'static, R: Send + Clone + 'static, P: Clone> domains::DomainServicer
    for DServicer<E, R, P>
{
    type Events = E;

    type Requests = R;

    type Platform = P;

    fn shell(
        &self,
    ) -> impl crate::domains::DomainShell<
        Platform = Self::Platform,
        Events = Self::Events,
        Requests = Self::Requests,
    > {
        self.domain_shell.clone()
    }

    fn serve(
        &mut self,
        domain: impl crate::domains::Domain<
            Events = Self::Events,
            Requests = Self::Requests,
            Platform = Self::Platform,
        >,
    ) -> domains::DomainResult<()> {
        self.process_incoming_request(domain);
        self.execution_service
            .schedule_serve()
            .expect("should have kickstart resolve");
        Ok(())
    }
}
