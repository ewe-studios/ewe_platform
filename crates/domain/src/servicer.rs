// Module implementing DomainService default implementations and related tests

use channels::{
    broadcast, executor,
    mspc::{self, ChannelError},
};
use futures::future;
use tracing::{error, info};

use std::sync;

use crate::{
    domains::{self, DomainErrors, DomainResult, NamedEvent, NamedRequest},
    pending_chan::{self, PendingChannelError},
};

pub struct DShell<E: Send + Clone + 'static, R: Send + Clone + 'static, P: Clone> {
    shell_platform: P,
    executor: sync::Arc<executor::Executor<NamedEvent<E>>>,
    event_broadcast: broadcast::Broadcast<NamedEvent<E>>,
    request_broadcast: broadcast::Broadcast<NamedRequest<R>>,
    incoming_request_sender: mspc::SendChannel<NamedRequest<R>>,
    incoming_event_sender: mspc::SendChannel<NamedEvent<E>>,
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
            incoming_event_sender: self.incoming_event_sender.clone(),
        }
    }
}

impl<E: Send + Clone + 'static, R: Send + Clone + 'static, P: Clone> domains::MasterShell
    for DShell<E, R, P>
{
    fn send_request(
        &mut self,
        req: NamedRequest<Self::Requests>,
    ) -> domains::DomainOpsResult<mspc::ReceiveChannel<NamedEvent<Self::Events>>, Self::Requests>
    {
        self.request_broadcast.broadcast(req.clone());

        let mut resolution_channel = self.response_registry.register(req.id());
        Ok(resolution_channel
            .1
            .take()
            .expect("should have receiving channel"))
    }

    fn send_others(
        &mut self,
        event: NamedEvent<Self::Events>,
    ) -> domains::DomainOpsResult<(), Self::Events> {
        self.event_broadcast.broadcast(event.clone());
        Ok(())
    }

    fn send_all(
        &mut self,
        event: NamedEvent<Self::Events>,
    ) -> domains::DomainOpsResult<(), Self::Events> {
        println!("Sending info!");
        self.incoming_event_sender
            .try_send(event.clone())
            .expect("send event");
        println!("Sent info!");
        self.event_broadcast.broadcast(event);
        println!("broacast info!");
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
        id: domains::Id,
    ) -> domains::DomainOpsResult<mspc::SendChannel<NamedEvent<Self::Events>>, domains::Id> {
        match self.response_registry.resolve(id.clone()) {
            Ok(sender) => Ok(sender),
            Err(pending_chan::PendingChannelError::NotFound(_)) => {
                Err(domains::DomainOpsErrors::NotFound(id))
            }
            Err(pending_chan::PendingChannelError::ClosedSender(_)) => {
                Err(domains::DomainOpsErrors::ClosedChannel(id))
            }
        }
    }

    fn do_request(
        &mut self,
        req: NamedRequest<Self::Requests>,
    ) -> domains::DomainOpsResult<mspc::ReceiveChannel<NamedEvent<Self::Events>>, Self::Requests>
    {
        // create resolution channel group, send the RetreiveChannel to the user.
        let mut resolution_channel = self.response_registry.register(req.id());
        match self.incoming_request_sender.try_send(req.clone()) {
            Ok(_) => Ok(resolution_channel
                .1
                .take()
                .expect("should have receiving channel")),
            Err(_) => Err(domains::DomainOpsErrors::UnableToSendRequest(req)),
        }
    }

    fn schedule<Fut>(
        &self,
        receiver: mspc::ReceiveChannel<NamedEvent<Self::Events>>,
        receiver_fn: impl FnOnce(mspc::ChannelResult<NamedEvent<Self::Events>>) -> Fut + 'static + Send,
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
    closer_channel: mspc::ChannelGroup<()>,
    execution_service: executor::ExecutionService<NamedEvent<E>>,
    incoming_request_receiver: mspc::ReceiveChannel<NamedRequest<R>>,
    incoming_event_receiver: mspc::ReceiveChannel<NamedEvent<E>>,
    response_registry: pending_chan::PendingChannelsRegistry<NamedEvent<E>>,
}

const DEFAULT_SUBSCRIBER_START_CAPACITY: usize = 10;

pub fn create<E: Send + Clone + 'static, R: Send + Clone + 'static, P: Clone>(
    shell_platform: P,
) -> DServicer<E, R, P> {
    let closer_channel = mspc::ChannelGroup::new();
    let (incoming_request_sender, incoming_request_receiver) = mspc::create();
    let (incoming_event_sender, incoming_event_receiver) = mspc::create();
    let (execution_service, executor) = executor::create();
    let event_broadcast = broadcast::create::<NamedEvent<E>>(DEFAULT_SUBSCRIBER_START_CAPACITY);
    let request_broadcast = broadcast::create::<NamedRequest<R>>(DEFAULT_SUBSCRIBER_START_CAPACITY);
    let response_registry = pending_chan::PendingChannelsRegistry::new();

    let executor_arc = sync::Arc::new(executor);

    DServicer {
        domain_shell: DShell {
            shell_platform,
            incoming_request_sender,
            incoming_event_sender,
            executor: executor_arc,
            request_broadcast: request_broadcast.clone(),
            event_broadcast: event_broadcast.clone(),
            response_registry: response_registry.clone(),
        },
        execution_service: execution_service,
        incoming_request_receiver,
        incoming_event_receiver,
        response_registry,
        closer_channel,
    }
}

impl<E: Send + Clone + 'static, R: Send + Clone + 'static, P: Clone> DServicer<E, R, P> {
    fn process_incoming_event(
        &mut self,
        domain: &impl domains::Domain<Events = E, Requests = R, Platform = P>,
    ) -> DomainResult<()> {
        match self.incoming_event_receiver.try_receive() {
            Ok(request) => {
                domain.handle_event(request, self.domain_shell.clone());
                Ok(())
            }
            Err(ChannelError::ReceivedNoData) => Ok(()),
            Err(ChannelError::ReceiveFailed(_)) => Err(DomainErrors::ClosedRequestReceiver),
            _ => Ok(()),
        }
    }

    fn process_incoming_request(
        &mut self,
        domain: &impl domains::Domain<Events = E, Requests = R, Platform = P>,
    ) -> DomainResult<()> {
        match self.incoming_request_receiver.try_receive() {
            Ok(request) => match self.response_registry.resolve(request.id()) {
                Ok(sender) => {
                    domain.handle_request(request, sender, self.domain_shell.clone());
                    Ok(())
                }
                Err(PendingChannelError::ClosedSender(_)) => {
                    Err(DomainErrors::UnexpectedSenderClosure)
                }
                Err(PendingChannelError::NotFound(_)) => Err(DomainErrors::RequestSenderNotFound),
            },
            Err(mspc::ChannelError::Closed) => Err(DomainErrors::ClosedRequestReceiver),
            _ => Ok(()),
        }
    }

    #[allow(dead_code)]
    fn close(&mut self) {
        if let Some(mut sender) = self.closer_channel.0.clone() {
            sender.block_send(()).expect("should have sent signal")
        }
        self.response_registry.clear();
    }
}

impl<E: Send + Clone + 'static, R: Send + Clone + 'static, P: Clone> domains::DomainServicer
    for DServicer<E, R, P>
{
    type Events = E;

    type Requests = R;

    type Platform = P;

    fn shell(&self) -> impl domains::DomainShell<Events = E, Requests = R, Platform = P> {
        self.domain_shell.clone()
    }

    fn serve(
        &mut self,
        domain: &impl crate::domains::Domain<
            Events = Self::Events,
            Requests = Self::Requests,
            Platform = Self::Platform,
        >,
    ) -> domains::DomainResult<()> {
        (match self.process_incoming_event(domain) {
            Ok(_) => Ok(()),
            Err(DomainErrors::RequestSenderNotFound) => Err(DomainErrors::ProblematicState),
            Err(DomainErrors::UnexpectedSenderClosure) => Err(DomainErrors::ProblematicState),
            _ => Ok(()),
        })
        .expect("event processing should have finished with no issues");
        (match self.process_incoming_request(domain) {
            Ok(_) => Ok(()),
            Err(DomainErrors::RequestSenderNotFound) => Err(DomainErrors::ProblematicState),
            Err(DomainErrors::UnexpectedSenderClosure) => Err(DomainErrors::ProblematicState),
            _ => Ok(()),
        })
        .expect("request processing should have finished with no issues");
        (match self.execution_service.schedule_serve() {
            Ok(_) => Ok(()),
            Err(executor::ExecutorError::Decommission) => Err(DomainErrors::ProblematicState),
            _ => Ok(()),
        })
        .expect("execution service should have ended better");
        Ok(())
    }

    fn serve_forever(
        &mut self,
        d: &impl domains::Domain<
            Events = Self::Events,
            Requests = Self::Requests,
            Platform = Self::Platform,
        >,
    ) -> domains::DomainResult<()> {
        info!("Starting domain servicer listening loop");
        loop {
            if let Some(mut recv) = self.closer_channel.1.clone() {
                if let Err(_) = recv.try_receive() {
                    self.serve(d).expect("should not have ended in this state");
                    continue;
                } else {
                    info!("domain servicer closure requested");
                    return Err(DomainErrors::CloseRequested);
                }
            }

            // generally this should not occur but its good to check
            error!("closing due to None closer channel in domain servicer");
            return Err(DomainErrors::ProblematicState);
        }
    }

    fn serve_forever_async(
        &mut self,
        d: &impl domains::Domain<
            Events = Self::Events,
            Requests = Self::Requests,
            Platform = Self::Platform,
        >,
    ) -> impl future::Future<Output = domains::DomainResult<()>> {
        async { self.serve_forever(d) }
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use crate::{
        app,
        domains::{self, DomainErrors, DomainOpsResult, DomainServicer, DomainShell},
        servicer,
    };
    use crossbeam::atomic;
    use std::sync;
    use tracing::info;

    #[test]
    fn can_create_service_run_and_close_it() {
        let counter_app = CounterApp::default();
        let domain_servicer =
            sync::Arc::new(sync::Mutex::new(servicer::create(Platform::default())));

        let thread_instance = domain_servicer.clone();
        let t = thread::spawn(move || {
            let mut instance = thread_instance.lock().unwrap();
            match instance.serve_forever(&counter_app) {
                Ok(_) => println!("Closed servicer!"),
                Err(DomainErrors::CloseRequested) => println!("Closing servicer!"),
                _ => panic!("bad ending"),
            };
        });

        domain_servicer.lock().unwrap().close();

        _ = t.join().expect("thread should have been closed correctly");
    }

    #[test]
    fn can_create_and_close_domain_service_without_starting() {
        let mut domain_servicer =
            servicer::create::<CounterEvents, CounterRequests, Platform>(Platform::default());
        domain_servicer.close();
    }

    #[test]
    fn can_decrement_count_with_a_decrement_requests() {
        let (app, mut server) = app::create::<CounterApp>();

        let request = domains::NamedRequest::new("decrement_count", CounterRequests::Decrement);

        let result;
        {
            result = server.shell().do_request(request)
        }

        server.serve(&app).expect("serve domain");

        assert!(matches!(result, DomainOpsResult::Ok(_)));

        let mut receiver = result.expect("expected a receiver");

        let item = receiver.block_receive().expect("should receive value");

        let items = item.items();

        assert_eq!(
            *items.first().take().unwrap(),
            CounterEvents::Decremented(CounterModel::new(-1))
        );

        server.serve(&app).expect("serve domain for next event");

        let mut events;
        let mut requests;
        {
            let mut shell = server.shell();
            events = shell.listen().unwrap();
            requests = shell.requests().unwrap();
        }

        let published_event = events.block_receive().expect("got event");

        assert_eq!(
            published_event.items(),
            vec![CounterEvents::Decremented(CounterModel::new(-1))]
        );

        let published_requests = requests.block_receive().expect("got requests");
        assert_eq!(
            published_requests.item(),
            CounterRequests::Render(CounterModel::new(-1))
        );
    }

    #[test]
    fn can_increment_count_with_an_increment_request() {
        let (app, mut server) = app::create::<CounterApp>();

        let increment_request =
            domains::NamedRequest::new("increment_count", CounterRequests::Increment);

        let result;
        {
            result = server.shell().do_request(increment_request)
        }

        server.serve(&app).expect("serve domain");

        assert!(matches!(result, DomainOpsResult::Ok(_)));

        let mut receiver = result.expect("expected a receiver");

        let item = receiver.block_receive().expect("should receive value");

        let items = item.items();

        assert_eq!(
            *items.first().take().unwrap(),
            CounterEvents::Incremented(CounterModel::new(1))
        );

        server.serve(&app).expect("serve domain for next event");

        let mut events;
        let mut requests;

        {
            let mut shell = server.shell();
            events = shell.listen().unwrap();
            requests = shell.requests().unwrap();
        }

        let published_event = events.block_receive().expect("got event");
        assert_eq!(
            published_event.items(),
            vec![CounterEvents::Incremented(CounterModel::new(1))]
        );

        let published_requests = requests.block_receive().expect("got requests");
        assert_eq!(
            published_requests.item(),
            CounterRequests::Render(CounterModel::new(1))
        );
    }

    #[derive(Default, Clone)]
    struct Platform {}

    #[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
    struct CounterModel {
        pub count: i16,
    }

    impl CounterModel {
        pub fn new(value: i16) -> Self {
            Self { count: value }
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum CounterEvents {
        Incremented(CounterModel),
        Decremented(CounterModel),
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum CounterRequests {
        Increment,
        Decrement,
        Render(CounterModel),
    }

    #[derive(Clone)]
    struct CounterApp {
        state: sync::Arc<atomic::AtomicCell<CounterModel>>,
    }

    impl Default for CounterApp {
        fn default() -> Self {
            Self {
                state: sync::Arc::new(atomic::AtomicCell::new(CounterModel { count: 0 })),
            }
        }
    }

    impl domains::Domain for CounterApp {
        type Events = CounterEvents;
        type Requests = CounterRequests;
        type Platform = Platform;

        fn handle_request(
            &self,
            req: domains::NamedRequest<Self::Requests>,
            mut chan: channels::mspc::SendChannel<domains::NamedEvent<Self::Events>>,
            mut shell: impl domains::MasterShell<
                Events = Self::Events,
                Requests = Self::Requests,
                Platform = Self::Platform,
            >,
        ) {
            match req.item() {
                CounterRequests::Increment => {
                    let current = self.state.load();
                    let next = CounterModel {
                        count: current.count + 1,
                    };
                    self.state.swap(next.clone());

                    let event = req.to_one(CounterEvents::Incremented(next));
                    chan.try_send(event.clone())
                        .expect("should have sent message");

                    shell
                        .send_all(event)
                        .expect("should notify interested parties on important change");
                }
                CounterRequests::Decrement => {
                    let current = self.state.load();
                    let next = CounterModel {
                        count: current.count - 1,
                    };
                    self.state.swap(next.clone());

                    // respond to request with new state via event
                    let event = req.to_one(CounterEvents::Decremented(next));
                    chan.try_send(event.clone())
                        .expect("should have sent message");

                    shell
                        .send_all(event)
                        .expect("should notify interested parties on important change");
                }
                CounterRequests::Render(_) => {}
            };
        }

        fn handle_event(
            &self,
            events: domains::NamedEvent<Self::Events>,
            mut shell: impl domains::MasterShell<
                Events = Self::Events,
                Requests = Self::Requests,
                Platform = Self::Platform,
            >,
        ) {
            for item in events.items() {
                match item {
                    CounterEvents::Incremented(model) => {
                        info!("incremented counter to {}", model.count);
                        shell
                            .send_request(domains::NamedRequest::new(
                                "render_count",
                                CounterRequests::Render(model),
                            ))
                            .expect("sent request");
                    }
                    CounterEvents::Decremented(model) => {
                        info!("decremented counter to {}", model.count);
                        shell
                            .send_request(domains::NamedRequest::new(
                                "render_count",
                                CounterRequests::Render(model),
                            ))
                            .expect("sent request");
                    }
                }
            }
        }
    }
}
