// DServicer integration tests using a small Counter domain.

use crossbeam::atomic;
use std::sync;

use ewe_domain::{
    app,
    domains::{self, DomainOpsResult, DomainShell},
    servicer,
};
use tracing::info;

#[test]
fn can_decrement_count_with_a_decrement_requests() {
    let (mut executor, server) = app::create::<CounterApp>();
    let mut shell = servicer::create_shell(&server);

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
    let mut shell = servicer::create_shell(&server);

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
        *items.first().unwrap(),
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
    let mut shell = servicer::create_shell(&server);

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
        }
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
