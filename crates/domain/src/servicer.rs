// Module implementing DomainService default implementations and related tests

use ewe_channels::{
    broadcast, executor,
    mspc::{self, ChannelError},
};

use std::sync;

use crate::{
    domains::{self, DomainErrors, DomainResult, NamedEvent, NamedRequest},
    pending_chan::{self, PendingChannelError},
};

const DEFAULT_SUBSCRIBER_START_CAPACITY: usize = 10;

pub fn create<
    App,
    E: Send + Clone + 'static,
    R: Send + Clone + 'static,
    P: Default + Clone + 'static,
>() -> DServicer<App, E, R, P>
where
    App: domains::Domain<Events = E, Requests = R, Platform = P>,
{
    let (incoming_request_sender, incoming_request_receiver) = mspc::create();
    let (incoming_event_sender, incoming_event_receiver) = mspc::create();
    let (execution_service, executor) = executor::create();
    let event_broadcast = broadcast::create::<NamedEvent<E>>(DEFAULT_SUBSCRIBER_START_CAPACITY);
    let request_broadcast = broadcast::create::<NamedRequest<R>>(DEFAULT_SUBSCRIBER_START_CAPACITY);
    let response_registry = pending_chan::PendingChannelsRegistry::new();

    let executor_arc = sync::Arc::new(executor);

    DServicer {
        domain_shell: DShell {
            incoming_request_sender,
            incoming_event_sender,
            executor: executor_arc,
            shell_platform: App::Platform::default(),
            request_broadcast: request_broadcast.clone(),
            event_broadcast: event_broadcast.clone(),
            response_registry: response_registry.clone(),
        },
        domain_provider: App::default(),
        incoming_request_receiver,
        incoming_event_receiver,
        response_registry,
        execution_service,
    }
}

pub struct DShell<
    E: Send + Clone + 'static,
    R: Send + Clone + 'static,
    P: Default + Clone + 'static,
> {
    shell_platform: P,
    executor: sync::Arc<executor::Executor<NamedEvent<E>>>,
    event_broadcast: broadcast::Broadcast<NamedEvent<E>>,
    request_broadcast: broadcast::Broadcast<NamedRequest<R>>,
    incoming_request_sender: mspc::SendChannel<NamedRequest<R>>,
    incoming_event_sender: mspc::SendChannel<NamedEvent<E>>,
    response_registry: pending_chan::PendingChannelsRegistry<NamedEvent<E>>,
}

impl<E: Send + Clone + 'static, R: Send + Clone + 'static, P: Default + Clone + 'static> Clone
    for DShell<E, R, P>
{
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

impl<E: Send + Clone + 'static, R: Send + Clone + 'static, P: Default + Clone + 'static>
    domains::MasterShell for DShell<E, R, P>
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
        self.incoming_event_sender
            .try_send(event.clone())
            .expect("send event");
        self.event_broadcast.broadcast(event);
        Ok(())
    }
}

impl<E: Send + Clone + 'static, R: Send + Clone + 'static, P: Default + Clone + 'static>
    domains::DomainShell for DShell<E, R, P>
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
    ) -> domains::DomainOpsResult<mspc::SendChannel<NamedEvent<Self::Events>>, domains::Id>
    where
        Self: Sized,
    {
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
    where
        Self: Sized,
    {
        // create resolution channel group, send the RetreiveChannel to the user.
        let mut resolution_channel = self.response_registry.register(req.id());
        match self.incoming_request_sender.try_send(req.clone()) {
            Ok(()) => Ok(resolution_channel
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
            Ok(()) => Ok(()),
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
            Ok(()) => Ok(()),
            Err(_) => Err(domains::DomainErrors::FailedScheduling),
        }
    }

    fn requests(
        &mut self,
    ) -> domains::DomainResult<mspc::ReceiveChannel<sync::Arc<NamedRequest<Self::Requests>>>>
    where
        Self: Sized,
    {
        Ok(self.request_broadcast.subscribe())
    }

    fn listen(
        &mut self,
    ) -> domains::DomainResult<mspc::ReceiveChannel<sync::Arc<NamedEvent<Self::Events>>>>
    where
        Self: Sized,
    {
        Ok(self.event_broadcast.subscribe())
    }
}

pub struct DServicer<
    App,
    E: Send + Clone + 'static,
    R: Send + Clone + 'static,
    P: Default + Clone + 'static,
> where
    App: domains::Domain<Events = E, Requests = R, Platform = P>,
{
    domain_provider: App,
    domain_shell: DShell<E, R, P>,
    execution_service: executor::ExecutionService<NamedEvent<E>>,
    incoming_request_receiver: mspc::ReceiveChannel<NamedRequest<R>>,
    incoming_event_receiver: mspc::ReceiveChannel<NamedEvent<E>>,
    response_registry: pending_chan::PendingChannelsRegistry<NamedEvent<E>>,
}

pub fn create_shell<
    App,
    E: Send + Clone + 'static,
    R: Send + Clone + 'static,
    P: Default + Clone + 'static,
>(
    servicer: DServicer<App, E, R, P>,
) -> DShell<E, R, P>
where
    App: domains::Domain<Events = E, Requests = R, Platform = P>,
{
    DShell {
        incoming_event_sender: servicer.domain_shell.incoming_event_sender.clone(),
        incoming_request_sender: servicer.domain_shell.incoming_request_sender.clone(),
        executor: servicer.domain_shell.executor.clone(),
        shell_platform: servicer.domain_shell.shell_platform.clone(),
        request_broadcast: servicer.domain_shell.request_broadcast.clone(),
        event_broadcast: servicer.domain_shell.event_broadcast.clone(),
        response_registry: servicer.domain_shell.response_registry.clone(),
    }
}

impl<A, E: Send + Clone + 'static, R: Send + Clone + 'static, P: Clone + Default + 'static> Clone
    for DServicer<A, E, R, P>
where
    A: domains::Domain<Events = E, Requests = R, Platform = P>,
{
    fn clone(&self) -> Self {
        Self {
            domain_provider: self.domain_provider.clone(),
            domain_shell: self.domain_shell.clone(),
            execution_service: self.execution_service.clone(),
            incoming_request_receiver: self.incoming_request_receiver.clone(),
            incoming_event_receiver: self.incoming_event_receiver.clone(),
            response_registry: self.response_registry.clone(),
        }
    }
}

impl<A, E: Send + Clone + 'static, R: Send + Clone + 'static, P: Clone + Default + 'static>
    DServicer<A, E, R, P>
where
    A: domains::Domain<Events = E, Requests = R, Platform = P>,
{
    fn process_incoming_event(&mut self) -> DomainResult<()> {
        match self.incoming_event_receiver.try_receive() {
            Ok(request) => {
                self.domain_provider
                    .handle_event(request, self.domain_shell.clone());
                Ok(())
            }
            Err(ChannelError::ReceivedNoData) => Ok(()),
            Err(ChannelError::ReceiveFailed(_)) => Err(DomainErrors::ClosedRequestReceiver),
            _ => Ok(()),
        }
    }

    fn process_incoming_request(&mut self) -> DomainResult<()> {
        match self.incoming_request_receiver.try_receive() {
            Ok(request) => match self.response_registry.resolve(request.id()) {
                Ok(sender) => {
                    self.domain_provider
                        .handle_request(request, sender, self.domain_shell.clone());
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

    #[allow(unused)]
    fn close(&mut self) {
        self.execution_service.close();
        self.response_registry.clear();
    }

    pub fn serve(&mut self) -> domains::DomainResult<()> {
        self.serve_events().expect("served events");
        self.serve_requests().expect("served requests");
        Ok(())
    }

    fn serve_events(&mut self) -> domains::DomainResult<()> {
        (match self.process_incoming_event() {
            Ok(()) => Ok(()),
            Err(DomainErrors::RequestSenderNotFound) => Err(DomainErrors::ProblematicState),
            Err(DomainErrors::UnexpectedSenderClosure) => Err(DomainErrors::ProblematicState),
            _ => Ok(()),
        })
        .expect("event processing should have finished with no issues");

        (match self.execution_service.schedule_serve() {
            Ok(()) => Ok(()),
            Err(executor::ExecutorError::Decommission) => Err(DomainErrors::ProblematicState),
            _ => Ok(()),
        })
        .expect("execution service should have ended better");

        Ok(())
    }

    fn serve_requests(&mut self) -> domains::DomainResult<()> {
        (match self.process_incoming_request() {
            Ok(()) => Ok(()),
            Err(DomainErrors::RequestSenderNotFound) => Err(DomainErrors::ProblematicState),
            Err(DomainErrors::UnexpectedSenderClosure) => Err(DomainErrors::ProblematicState),
            _ => Ok(()),
        })
        .expect("request processing should have finished with no issues");

        (match self.execution_service.schedule_serve() {
            Ok(()) => Ok(()),
            Err(executor::ExecutorError::Decommission) => Err(DomainErrors::ProblematicState),
            _ => Ok(()),
        })
        .expect("execution service should have ended better");

        Ok(())
    }
}

impl<A, E: Send + Clone + 'static, R: Send + Clone + 'static, P: Default + Clone + 'static>
    domains::TaskExecutor for DServicer<A, E, R, P>
where
    A: domains::Domain<Events = E, Requests = R, Platform = P>,
{
    fn run_tasks(&mut self) {
        self.serve().expect("execute all tasks with no errors");
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        app,
        domains::{self, DomainOpsResult, DomainShell},
        servicer,
    };
    use crossbeam::atomic;
    use std::sync;
    use tracing::info;

    #[test]
    fn can_decrement_count_with_a_decrement_requests() {
        let (mut executor, server) = app::create::<CounterApp>();
        let mut shell = servicer::create_shell(server);

        let request = domains::NamedRequest::new("decrement_count", CounterRequests::Decrement);

        let result = shell.do_request(request);

        executor.run_all();

        assert!(matches!(result, DomainOpsResult::Ok(_)));

        let mut receiver = result.expect("expected a receiver");

        let item = receiver.block_receive().expect("should receive value");

        let items = item.items();

        assert_eq!(
            *items.first().unwrap(),
            CounterEvents::Decremented(CounterModel::new(-1))
        );

        executor.run_all();

        let mut events = shell.listen().unwrap();
        let mut requests = shell.requests().unwrap();

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
        let (mut executor, server) = app::create::<CounterApp>();
        let mut shell = servicer::create_shell(server);

        let increment_request =
            domains::NamedRequest::new("increment_count", CounterRequests::Increment);

        let result;
        {
            result = shell.do_request(increment_request);
        }

        executor.run_all();

        assert!(matches!(result, DomainOpsResult::Ok(_)));

        let mut receiver = result.expect("expected a receiver");

        let item = receiver.block_receive().expect("should receive value");

        let items = item.items();

        assert_eq!(
            *items.first().take().unwrap(),
            CounterEvents::Incremented(CounterModel::new(1))
        );

        executor.run_all();

        let mut events = shell.listen().unwrap();
        let mut requests = shell.requests().unwrap();

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

    #[test]
    fn can_use_use_case_implementation_with_an_app() {
        let (mut executor, server) = app::create::<CounterApp>();
        let mut shell = servicer::create_shell(server);

        let increment_request =
            domains::NamedRequest::new("increment_count", CounterRequests::Increment);

        let result = shell.do_request(increment_request);

        executor.run_all();

        assert!(matches!(result, DomainOpsResult::Ok(_)));

        // call because a new data is waiting processing
        executor.run_all();

        let count_render = CounterRender::new();

        executor.register(Box::new(domains::UseCaseExecutor::new(
            shell,
            count_render.clone(),
        )));

        executor.run_all();

        assert!(!count_render.data.lock().unwrap().is_empty());
    }

    #[derive(Default, Clone)]
    struct Platform {}

    #[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
    struct CounterModel {
        pub count: i16,
    }

    #[derive(Clone)]
    struct CounterRender {
        pub data: sync::Arc<sync::Mutex<Vec<String>>>,
    }

    impl CounterRender {
        pub fn new() -> Self {
            Self {
                data: sync::Arc::new(sync::Mutex::new(Vec::new())),
            }
        }
    }

    impl domains::UseCase for CounterRender {
        type Platform = Platform;
        type Event = CounterEvents;
        type Request = CounterRequests;

        fn is_request(&self, req: sync::Arc<domains::NamedRequest<Self::Request>>) -> bool {
            if let CounterRequests::Render(_) = req.item() {
                return true;
            }
            false
        }

        fn handle_request(
            &mut self,
            req: sync::Arc<domains::NamedRequest<Self::Request>>,
            mut chan: ewe_channels::mspc::SendChannel<domains::NamedEvent<Self::Event>>,
            _shell: impl DomainShell<
                Events = Self::Event,
                Requests = Self::Request,
                Platform = Self::Platform,
            >,
        ) {
            if let CounterRequests::Render(model) = req.item() {
                self.data
                    .lock()
                    .unwrap()
                    .push(format!("Counter(count: {})", model.count));
            }
            chan.close().expect("close channel");
        }
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
            mut chan: ewe_channels::mspc::SendChannel<domains::NamedEvent<Self::Events>>,
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
                    self.state.swap(next);

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
                    self.state.swap(next);

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
