//! The tests of the Durable Object control plane.

use std::time::Duration;

use foundation_netio::shared::http::{SimpleHeader, SimpleIncomingRequest, SimpleMethod};
use foundation_sandbox::shared::cloudflare::testing::{
    control_exec_request, control_get_request, response_body, response_status,
};
use foundation_sandbox::shared::cloudflare::ContainerApi;
use foundation_sandbox::shared::cloudflare::DoControlPlane;
use foundation_sandbox::shared::command::Command;
use foundation_sandbox::shared::control::SandboxState;
use foundation_sandbox::shared::driver::SlaveEndpoint;
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::rpc::messages::{ExecEventKind, SandboxInfo, SandboxRef};

use super::support::{self, block_on, spec, Harness};

/// Restores the sandbox through one `Get` request.
fn get(
    planner: &DoControlPlane,
    id: &foundation_sandbox::shared::ids::SandboxId,
) -> (u16, Vec<u8>) {
    let response = block_on(planner.handle(control_get_request(id)));
    (response_status(&response), response_body(response))
}

#[test]
fn one_sandbox_per_do() {
    let h = Harness::new(20, true, false);
    let planner = support::planner(&h, Some(h.id()));
    block_on(planner.sandboxes().create(spec())).expect("create");
    let error = support::error_of(
        block_on(planner.sandboxes().create(spec())),
        "a second create fails",
    );
    assert!(matches!(error, SandboxError::TooManySessions));
}

#[test]
fn restore_live() {
    let h = Harness::new(21, true, false);
    {
        let planner = support::planner(&h, Some(h.id()));
        block_on(planner.sandboxes().create(spec())).expect("create");
    }
    h.scripted.with_exec_events(vec![
        foundation_sandbox::shared::rpc::messages::ExecEventKind::Started { pid: 1 },
        foundation_sandbox::shared::rpc::messages::ExecEventKind::Stdout {
            data: b"again\n".to_vec(),
        },
        foundation_sandbox::shared::rpc::messages::ExecEventKind::Exit {
            code: Some(0),
            signal: None,
        },
    ]);
    let planner = support::planner(&h, None);
    let (status, body) = get(&planner, &h.id());
    assert_eq!(status, 200);
    let info: SandboxInfo = serde_json::from_slice(&body).expect("the info is JSON");
    assert_eq!(info.state, SandboxState::Ready);
    assert_eq!(h.container.start_count(), 1);
    let sandbox = block_on(planner.sandboxes().get(h.id())).expect("get");
    let output = block_on(sandbox.run(Command::new(["echo", "again"]))).expect("run");
    assert_eq!(output.stdout, b"again\n");
}

#[test]
fn restore_dead_container() {
    let h = Harness::new(22, true, false);
    let record = foundation_sandbox::shared::driver::StoredRecord {
        id: h.id(),
        driver: foundation_sandbox::shared::ids::DriverId::CloudflareContainer,
        spec: spec(),
        state: SandboxState::Ready,
        created_unix_ms: 0,
        endpoint: Some(SlaveEndpoint::CloudflarePort { port: 1024 }),
    };
    h.storage.insert(
        foundation_sandbox::shared::cloudflare::store::SANDBOX_KEY,
        serde_json::to_vec(&record).expect("the record is JSON"),
    );
    h.storage.insert(
        foundation_sandbox::shared::cloudflare::control::SANDBOX_ID_KEY,
        h.id().to_string().into_bytes(),
    );
    let planner = support::planner(&h, None);
    let error = support::error_of(
        block_on(planner.sandboxes().get(h.id())),
        "the sandbox is gone",
    );
    assert!(matches!(error, SandboxError::NotFound { .. }));
    let (status, _body) = get(&planner, &h.id());
    assert_ne!(status, 200);
}

#[test]
fn restore_unprepared_slave() {
    let h = Harness::new(23, true, false);
    h.container
        .start(foundation_sandbox::shared::cloudflare::ContainerStart {
            entrypoint: h.host.config.slave_entrypoint(),
            enable_internet: false,
            env: Vec::new(),
        })
        .expect("start");
    h.scripted.with_info_prepared(false);
    let record = foundation_sandbox::shared::driver::StoredRecord {
        id: h.id(),
        driver: foundation_sandbox::shared::ids::DriverId::CloudflareContainer,
        spec: spec(),
        state: SandboxState::Ready,
        created_unix_ms: 0,
        endpoint: Some(SlaveEndpoint::CloudflarePort { port: 1024 }),
    };
    h.storage.insert(
        foundation_sandbox::shared::cloudflare::store::SANDBOX_KEY,
        serde_json::to_vec(&record).expect("the record is JSON"),
    );
    h.storage.insert(
        foundation_sandbox::shared::cloudflare::control::SANDBOX_ID_KEY,
        h.id().to_string().into_bytes(),
    );
    let planner = support::planner(&h, None);
    let (status, _body) = get(&planner, &h.id());
    assert_ne!(status, 200);
    assert_eq!(
        h.storage
            .value(foundation_sandbox::shared::cloudflare::store::SANDBOX_KEY),
        None
    );
}

#[test]
fn restore_creating_record() {
    let h = Harness::new(24, true, false);
    h.container
        .start(foundation_sandbox::shared::cloudflare::ContainerStart {
            entrypoint: h.host.config.slave_entrypoint(),
            enable_internet: false,
            env: Vec::new(),
        })
        .expect("start");
    let record = foundation_sandbox::shared::driver::StoredRecord {
        id: h.id(),
        driver: foundation_sandbox::shared::ids::DriverId::CloudflareContainer,
        spec: spec(),
        state: SandboxState::Creating,
        created_unix_ms: 0,
        endpoint: None,
    };
    h.storage.insert(
        foundation_sandbox::shared::cloudflare::store::SANDBOX_KEY,
        serde_json::to_vec(&record).expect("the record is JSON"),
    );
    h.storage.insert(
        foundation_sandbox::shared::cloudflare::control::SANDBOX_ID_KEY,
        h.id().to_string().into_bytes(),
    );
    let planner = support::planner(&h, None);
    let (status, _body) = get(&planner, &h.id());
    assert_ne!(status, 200);
    assert!(!h.container.is_running());
    assert_eq!(
        h.storage
            .value(foundation_sandbox::shared::cloudflare::store::SANDBOX_KEY),
        None
    );
}

#[test]
fn alarm_on_fresh_object_stops() {
    let h = Harness::new(25, true, false);
    {
        let planner = support::planner(&h, Some(h.id()));
        block_on(planner.sandboxes().create(spec())).expect("create");
        let _ = get(&planner, &h.id());
    }
    h.clock.advance(Duration::from_secs(600));
    h.clock.advance_wall(Duration::from_secs(600));
    let planner = support::planner(&h, None);
    block_on(planner.on_alarm());
    assert!(!h.container.is_running());
    assert!(h.container.destroy_count() > 0);
    assert_eq!(h.alarm.value(), None);
    assert_eq!(
        h.storage
            .value(foundation_sandbox::shared::cloudflare::store::SANDBOX_KEY),
        None
    );
}

#[test]
fn idle_stops() {
    let h = Harness::new(26, true, false);
    let planner = support::planner(&h, Some(h.id()));
    block_on(planner.sandboxes().create(spec())).expect("create");
    let _ = get(&planner, &h.id());
    h.clock.advance(Duration::from_secs(600));
    h.clock.advance_wall(Duration::from_secs(600));
    block_on(planner.on_alarm());
    assert!(!h.container.is_running());
    assert!(h.container.destroy_count() > 0);
    assert_eq!(h.alarm.value(), None);
    assert!(h.index.is_empty());
}

#[test]
fn touch_moves_deadline() {
    let h = Harness::new(27, true, false);
    let planner = support::planner(&h, Some(h.id()));
    block_on(planner.sandboxes().create(spec())).expect("create");
    let _ = get(&planner, &h.id());
    h.clock.advance_wall(Duration::from_secs(540));
    let _ = get(&planner, &h.id());
    h.clock.advance_wall(Duration::from_secs(60));
    h.clock.advance(Duration::from_secs(60));
    block_on(planner.on_alarm());
    assert!(h.container.is_running());
    assert_eq!(h.alarm.value(), Some(19 * 60 * 1000));
    let (status, body) = get(&planner, &h.id());
    assert_eq!(status, 200);
    let info: SandboxInfo = serde_json::from_slice(&body).expect("the info is JSON");
    assert_eq!(info.state, SandboxState::Ready);
}

#[test]
fn stream_guard_touches() {
    let h = Harness::new(28, true, false);
    let planner = support::planner(&h, Some(h.id()));
    block_on(planner.sandboxes().create(spec())).expect("create");
    let _ = get(&planner, &h.id());
    let before = h.alarm.value();
    let guard = planner.stream_guard();
    std::thread::sleep(Duration::from_millis(20));
    assert!(guard.touches() >= 1);
    assert!(h.alarm.value().is_some());
    assert!(before.is_some());
    drop(guard);
}

#[test]
fn stop_removes_all() {
    let h = Harness::new(29, true, false);
    let planner = support::planner(&h, Some(h.id()));
    let sandbox = block_on(planner.sandboxes().create(spec())).expect("create");
    block_on(sandbox.stop()).expect("stop");
    assert!(!h.container.is_running());
    assert!(h.container.destroy_count() > 0);
    assert_eq!(
        h.storage
            .value(foundation_sandbox::shared::cloudflare::store::SANDBOX_KEY),
        None
    );
    assert!(h.index.is_empty());
    assert_eq!(h.alarm.value(), None);
}

#[test]
fn index_error_does_not_fail_create() {
    let h = Harness::new(30, true, false).with_failing_index();
    let planner = support::planner(&h, Some(h.id()));
    block_on(planner.sandboxes().create(spec())).expect("the create succeeds");
}

#[test]
fn container_exit_fails_sandbox() {
    let h = Harness::new(31, true, false);
    let planner = support::planner(&h, Some(h.id()));
    let sandbox = block_on(planner.sandboxes().create(spec())).expect("create");
    h.container.exit();
    for _ in 0..50 {
        if h.storage
            .value(foundation_sandbox::shared::cloudflare::store::SANDBOX_KEY)
            .is_none()
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        h.storage
            .value(foundation_sandbox::shared::cloudflare::store::SANDBOX_KEY),
        None
    );
    let listed = block_on(planner.sandboxes().list()).expect("list");
    assert!(listed.is_empty(), "{listed:?}");
    let _ = sandbox;
}

#[test]
fn idle_model() {
    // A stateful check: random requests and alarms against one deadline.
    let h = Harness::new(32, true, false);
    let planner = support::planner(&h, Some(h.id()));
    block_on(planner.sandboxes().create(spec())).expect("create");
    let mut rng = 0x2545_f491_4f6c_dd1d_u64;
    for _ in 0..8 {
        rng = rng.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let step = (rng >> 33) % 200;
        h.clock.advance_wall(Duration::from_secs(step));
        let _ = get(&planner, &h.id());
        let deadline = h.alarm.value().expect("an alarm exists");
        assert!(deadline > 0);
    }
    assert!(h.container.is_running());
}

#[test]
fn handle_routes_get() {
    let h = Harness::new(33, true, false);
    let planner = support::planner(&h, Some(h.id()));
    block_on(planner.sandboxes().create(spec())).expect("create");
    let (status, body) = get(&planner, &h.id());
    assert_eq!(status, 200);
    let info: SandboxInfo = serde_json::from_slice(&body).expect("the info is JSON");
    assert_eq!(info.id, h.id().to_string());
}

#[test]
fn handle_streams_exec() {
    let h = Harness::new(34, true, false);
    let planner = support::planner(&h, Some(h.id()));
    let sandbox = block_on(planner.sandboxes().create(spec())).expect("create");
    h.scripted.with_gated_exec(
        vec![
            ExecEventKind::Started { pid: 1 },
            ExecEventKind::Stdout {
                data: b"first".to_vec(),
            },
        ],
        ExecEventKind::Exit {
            code: Some(0),
            signal: None,
        },
    );
    let exec_id = "0123456789abcdef";
    let mut response = block_on(planner.handle(control_exec_request(&h.id(), exec_id, false)));
    assert_eq!(response.status, 200);
    // The first frame is already in the body stream while the producer waits
    // on its gate, so the head left before the stream ended.
    let first = block_on(futures::StreamExt::next(&mut response.body))
        .expect("a first chunk")
        .expect("the first chunk is valid");
    assert_ne!(first.len(), 0);
    assert_eq!(h.scripted.signals().len(), 0);
    block_on(sandbox.calls().exec_signal(
        foundation_sandbox::shared::rpc::messages::ExecSignalRequest {
            exec_id: exec_id.to_string(),
            signal: 9,
        },
    ))
    .expect("release the producer");
    assert_eq!(
        h.scripted.signals(),
        vec![9],
        "the signal reached the slave"
    );
    let mut frames = 1;
    while let Some(item) = block_on(futures::StreamExt::next(&mut response.body)) {
        item.expect("a later chunk is valid");
        frames += 1;
    }
    assert!(frames > 1, "the stream has the later frames");
}

/// The Connect router looks up the request URL as a path.
///
/// The Worker entry must pass the path alone. An absolute URL misses every
/// route and answers an empty 404 (#988).
#[test]
fn router_matches_the_path_not_an_absolute_url() {
    let h = Harness::new(31, true, false);
    let planner = support::planner(&h, Some(h.id()));
    block_on(planner.sandboxes().create(spec())).expect("create");
    let path = "/ewe.sandbox.control.v1.SandboxService/Get";
    let body = serde_json::to_vec(&SandboxRef {
        sandbox_id: h.id().to_string(),
    })
    .expect("the request body");
    let request = |url: String| {
        SimpleIncomingRequest::builder()
            .with_plain_url(url)
            .with_method(SimpleMethod::POST)
            .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
            .with_body_bytes(body.clone())
            .build()
            .expect("the request")
    };

    let absolute = block_on(planner.handle(request(format!("http://worker.local{path}"))));
    assert_eq!(
        response_status(&absolute),
        404,
        "an absolute URL is not a router path"
    );
    let relative = block_on(planner.handle(request(path.to_string())));
    assert_eq!(
        response_status(&relative),
        200,
        "the path alone reaches the router"
    );
}
