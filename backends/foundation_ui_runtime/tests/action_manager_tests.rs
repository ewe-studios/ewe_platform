use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context as TaskContext, Poll, Waker};

use foundation_ui_runtime::action::{
    Action, ActionFuture, ActionManager, ActionRegistry, ActionTransport, Context, Interaction,
    ManagerError, Origin, Scope, Surface, TransportFuture, TriggerEvent,
};
use foundation_ui_wire::{
    read_frame, ActionInputT, ServerActionT, ServerCapsT, WireError, WireLimits, SCHEMA_VERSION,
};
use serde::Deserialize;

const REPLY: &[u8] = include_bytes!("../../foundation_ui_wire/fixtures/frame_ops_counter.bin");

#[derive(Default)]
struct RecordingTransport {
    requests: Mutex<Vec<Vec<u8>>>,
    response: Option<Vec<u8>>,
}

impl ActionTransport for RecordingTransport {
    fn send(&self, request: Vec<u8>) -> TransportFuture<'_> {
        self.requests.lock().unwrap().push(request);
        Box::pin(std::future::ready(Ok(self.response.clone())))
    }
}

struct Save;

impl Action for Save {
    type State = ();
    const NAME: &'static str = "save";
    fn origin() -> Option<Origin> {
        None
    }
    fn scope() -> Scope {
        Scope::All
    }
    fn call<'a>(&'a self, _: &'a Context, _: Interaction<()>) -> ActionFuture<'a> {
        Box::pin(async { Ok(()) })
    }
}

fn event() -> TriggerEvent {
    let mut input = ActionInputT::default();
    input.event_name = Some("submit".into());
    input.value = Some("Buy milk".into());
    input.parent_node = 40;
    input.checked = true;
    input.key_code = 13;
    input.modifiers = 2;
    TriggerEvent {
        node_id: 42,
        input,
        state: vec![0, 2, 255],
    }
}

fn caps(advertised: bool) -> ServerCapsT {
    let mut caps = ServerCapsT::default();
    let mut entry = ServerActionT::default();
    entry.name = Some("save".into());
    caps.actions = Some(if advertised { vec![entry] } else { Vec::new() });
    caps
}

// Recording transports complete immediately. A pending future indicates a failed test assumption.
fn immediate<T>(future: impl Future<Output = T>) -> T {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut TaskContext::from_waker(Waker::noop()))
    {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("Expected an immediate result"),
    }
}

#[derive(Deserialize, Debug)]
struct Route {
    reference: String,
    declaration: String,
    scope: String,
    surface: String,
    transports: Vec<String>,
    advertised: bool,
    expect: String,
}

#[test]
fn shared_route_vectors_submit_exactly_once_or_reject_before_send() {
    let routes: Vec<Route> =
        serde_json::from_str(include_str!("../fixtures/action_routes.json")).unwrap();
    for route in routes {
        let scope = match route.scope.as_str() {
            "all" => Scope::All,
            "web" => Scope::Web,
            "mobile" => Scope::Mobile,
            other => panic!("Unknown scope {other}"),
        };
        let surface = match route.surface.as_str() {
            "web" => Surface::Web,
            "mobile" => Surface::Native,
            other => panic!("Unknown surface {other}"),
        };
        let mut registry = ActionRegistry::new();
        match route.declaration.as_str() {
            "none" => {}
            "server" => registry
                .declare(Some(Origin::Server), "save", scope)
                .unwrap(),
            "host" => registry.declare(Some(Origin::Host), "save", scope).unwrap(),
            "plain" => registry.declare(None, "save", scope).unwrap(),
            "plain-local" => registry.register(Save).unwrap(),
            "page" => registry.register_page("save").unwrap(),
            other => panic!("Unknown declaration {other}"),
        }
        let mut manager =
            ActionManager::new(Arc::new(registry), surface, 11, WireLimits::default());
        let host = Arc::new(RecordingTransport::default());
        let server = Arc::new(RecordingTransport {
            response: Some(REPLY.to_vec()),
            ..RecordingTransport::default()
        });
        for transport in &route.transports {
            manager = match transport.as_str() {
                "host" => manager.with_host(host.clone()),
                "server" => manager.with_server(server.clone()),
                other => panic!("Unknown transport {other}"),
            };
        }
        let mut negotiated = caps(route.advertised);
        negotiated.stream_id = 71;
        manager.apply_caps(&negotiated);
        let result = immediate(manager.trigger(&route.reference, event()));
        let host_requests = host.requests.lock().unwrap();
        let server_requests = server.requests.lock().unwrap();
        let received = host_requests.len() + server_requests.len();
        match result {
            Err(ManagerError::Lookup(error)) => {
                assert_eq!(format!("{error:?}"), route.expect, "{route:?}");
                assert_eq!(received, 0, "{route:?}");
            }
            Err(other) => panic!("Unexpected failure for {route:?}: {other}"),
            Ok(answer) => {
                assert_eq!(received, 1, "{route:?}");
                let bytes = if route.expect.starts_with("server:") {
                    assert_eq!(answer.as_deref(), Some(REPLY));
                    &server_requests[0]
                } else {
                    assert!(answer.is_none());
                    &host_requests[0]
                };
                let frame = read_frame(bytes, &WireLimits::default()).unwrap();
                assert_eq!(
                    (
                        frame.schema_version(),
                        frame.stream_id(),
                        frame.sequence_id()
                    ),
                    (
                        SCHEMA_VERSION,
                        if route.expect.starts_with("server:") {
                            71
                        } else {
                            11
                        },
                        1
                    )
                );
                let request = frame.kind_as_frame_request().unwrap();
                assert!(request.hello().is_none());
                let action = request.action().unwrap().unpack();
                assert_eq!(action.action.as_deref(), Some(route.expect.as_str()));
                assert_eq!(action.node_id, 42);
                assert_eq!(action.input.as_deref(), Some(&event().input));
                assert_eq!(action.state, Some(event().state));
            }
        }
    }
}

#[test]
fn rejects_oversized_content_and_frame_overhead_before_submission() {
    for limit in [1, 48] {
        let transport = Arc::new(RecordingTransport::default());
        let mut registry = ActionRegistry::new();
        registry.register(Save).unwrap();
        let manager = ActionManager::new(
            Arc::new(registry),
            Surface::Native,
            1,
            WireLimits::default().max_action_request_bytes(limit),
        )
        .with_host(transport.clone());
        let result = immediate(manager.trigger("host:save", event()));
        assert!(
            matches!(result, Err(ManagerError::Wire(WireError::TooLarge { limit: n, actual })) if n == limit && actual > limit)
        );
        assert!(transport.requests.lock().unwrap().is_empty());
    }
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn concurrent_triggers_submit_in_sequence_order() {
    let transport = Arc::new(RecordingTransport::default());
    let mut registry = ActionRegistry::new();
    registry.register(Save).unwrap();
    let manager = ActionManager::new(
        Arc::new(registry),
        Surface::Native,
        5,
        WireLimits::default(),
    )
    .with_host(transport.clone());
    let barrier = std::sync::Barrier::new(8);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                barrier.wait();
                immediate(manager.trigger("host:save", event())).unwrap();
            });
        }
    });
    let requests = transport.requests.lock().unwrap();
    let sequence: Vec<_> = requests
        .iter()
        .map(|bytes| {
            read_frame(bytes, &WireLimits::default())
                .unwrap()
                .sequence_id()
        })
        .collect();
    assert_eq!(sequence, [1, 2, 3, 4, 5, 6, 7, 8]);
}

struct PendingTransport;
impl ActionTransport for PendingTransport {
    fn send(&self, _: Vec<u8>) -> TransportFuture<'_> {
        Box::pin(std::future::pending())
    }
}

#[test]
fn an_outstanding_reply_allows_another_action_to_start() {
    let mut registry = ActionRegistry::new();
    registry.register(Save).unwrap();
    let server = Arc::new(RecordingTransport::default());
    let manager = ActionManager::new(Arc::new(registry), Surface::Web, 3, WireLimits::default())
        .with_host(Arc::new(PendingTransport))
        .with_server(server.clone());
    manager.apply_caps(&caps(true));
    let mut first = std::pin::pin!(manager.trigger("host:save", event()));
    assert!(first
        .as_mut()
        .poll(&mut TaskContext::from_waker(Waker::noop()))
        .is_pending());
    immediate(manager.trigger("server:save", event())).unwrap();
    let requests = server.requests.lock().unwrap();
    assert_eq!(
        read_frame(&requests[0], &WireLimits::default())
            .unwrap()
            .sequence_id(),
        2
    );
}

struct FailingTransport(std::sync::atomic::AtomicUsize);

impl ActionTransport for FailingTransport {
    fn send(&self, _: Vec<u8>) -> TransportFuture<'_> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Box::pin(std::future::ready(Err(ManagerError::Transport(
            "deadline expired".into(),
        ))))
    }
}

#[test]
fn transport_failure_is_reported_once_and_a_new_advertisement_replaces_the_old_one() {
    use foundation_ui_runtime::action::LookupError;
    let mut registry = ActionRegistry::new();
    registry.declare(None, "save", Scope::All).unwrap();
    let transport = Arc::new(FailingTransport(std::sync::atomic::AtomicUsize::new(0)));
    let manager = ActionManager::new(Arc::new(registry), Surface::Web, 3, WireLimits::default())
        .with_server(transport.clone());
    assert!(matches!(
        immediate(manager.trigger("host:save", event())),
        Err(ManagerError::Lookup(LookupError::UnavailableOrigin))
    ));
    manager.apply_caps(&caps(true));
    assert!(
        matches!(immediate(manager.trigger("host:save", event())), Err(ManagerError::Transport(detail)) if detail == "deadline expired")
    );
    manager.apply_caps(&caps(false));
    assert!(matches!(
        immediate(manager.trigger("host:save", event())),
        Err(ManagerError::Lookup(LookupError::UnavailableOrigin))
    ));
    assert_eq!(transport.0.load(std::sync::atomic::Ordering::Relaxed), 1);
}

#[test]
fn stateless_control_omits_the_signed_state_field() {
    let mut registry = ActionRegistry::new();
    registry.register(Save).unwrap();
    let host = Arc::new(RecordingTransport::default());
    let manager = ActionManager::new(Arc::new(registry), Surface::Web, 3, WireLimits::default())
        .with_host(host.clone());
    let mut click = event();
    click.state.clear();
    immediate(manager.trigger("host:save", click)).unwrap();
    let requests = host.requests.lock().unwrap();
    let frame = read_frame(&requests[0], &WireLimits::default()).unwrap();
    assert!(frame
        .kind_as_frame_request()
        .unwrap()
        .action()
        .unwrap()
        .state()
        .is_none());
}

#[test]
fn structured_input_requires_negotiation_for_remote_calls() {
    use foundation_ui_wire::vocab::STRUCTURED_INPUT_VERSION;
    let mut registry = ActionRegistry::new();
    registry.register(Save).unwrap();
    let host = Arc::new(RecordingTransport::default());
    let server = Arc::new(RecordingTransport::default());
    let manager = ActionManager::new(Arc::new(registry), Surface::Web, 7, WireLimits::default())
        .with_host(host.clone())
        .with_server(server.clone());
    let payload = serde_json::json!({"list":{"id":7},"title":"Buy milk"});
    let mut advertised = caps(true);
    for version in [None, Some(STRUCTURED_INPUT_VERSION - 1)] {
        if let Some(v) = version {
            advertised.vocabulary_version = v;
            manager.apply_caps(&advertised);
        }
        assert!(matches!(
            immediate(manager.trigger_structured("server:save", event(), &payload)),
            Err(ManagerError::UnsupportedInput)
        ));
    }
    assert!(server.requests.lock().unwrap().is_empty());
    immediate(manager.trigger_structured("host:save", event(), &payload)).unwrap();
    advertised.vocabulary_version = STRUCTURED_INPUT_VERSION;
    manager.apply_caps(&advertised);
    immediate(manager.trigger_structured("server:save", event(), &payload)).unwrap();
    for transport in [host, server] {
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        let frame = read_frame(&requests[0], &WireLimits::default()).unwrap();
        let input = frame
            .kind_as_frame_request()
            .unwrap()
            .action()
            .unwrap()
            .input()
            .unwrap();
        assert!(input.value().is_none());
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(input.json().unwrap().bytes()).unwrap(),
            payload
        );
    }
}
