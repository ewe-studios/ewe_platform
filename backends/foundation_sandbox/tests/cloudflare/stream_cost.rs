//! The stream time limit of the Durable Object control plane (feature 18).
//!
//! Each test takes its own pool lease through [`super::support::block_on`].

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use foundation_connectrpc::{EndStreamResponse, Envelope};
use foundation_context::{Clock, Ctx};
use foundation_sandbox::shared::cloudflare::control::IDLE_DEADLINE_KEY;
use foundation_sandbox::shared::cloudflare::testing::control_exec_request;
use foundation_sandbox::shared::cloudflare::DoControlPlane;
use foundation_sandbox::shared::rpc::messages::ExecEventKind;

use super::support::{self, block_on, spec, Harness};

/// The idle timeout of the test configuration, in whole milliseconds.
const IDLE_MS: u64 = 600_000;

/// Waits until the stored idle deadline is at least `at_least`.
///
/// The stream guard touches the sandbox from a pool task, so the test waits
/// for the effect of the touch instead of for a fixed time.
fn wait_for_idle_deadline(h: &Harness, at_least: u64) {
    for _ in 0..500 {
        let deadline = h
            .storage
            .value(IDLE_DEADLINE_KEY)
            .and_then(|value| String::from_utf8(value).ok())
            .and_then(|text| text.parse::<u64>().ok());
        if deadline.is_some_and(|deadline| deadline >= at_least) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the stream guard did not move the idle deadline");
}

#[test]
fn stream_cap_ends_with_deadline() {
    // The timer of the simulated Env wakes when the clock moves.
    let h = Harness::new_simulated(120);
    let planner = support::planner(&h, Some(h.id()));
    block_on(planner.sandboxes().create(spec())).expect("create");
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
    // The head left before the stream ended, and the first frame is already in
    // the body. The program then waits at its gate, so the stream never ends.
    let first = block_on(futures::StreamExt::next(&mut response.body))
        .expect("a first chunk")
        .expect("the first chunk is valid");
    assert_ne!(first.len(), 0);

    // Step the clock past the limit of one stream.
    h.clock
        .advance(h.host.config.stream_max_duration + Duration::from_secs(1));
    let mut chunks = Vec::new();
    while let Some(item) = block_on(futures::StreamExt::next(&mut response.body)) {
        chunks.push(item.expect("a later chunk is valid"));
    }

    let last = chunks.pop().expect("the stream ends with one frame");
    let (envelope, _used) =
        Envelope::decode(&Bytes::from(last)).expect("the last chunk is a frame");
    assert!(envelope.is_end_stream(), "the frame is an end stream");
    let end: EndStreamResponse =
        serde_json::from_slice(&envelope.data).expect("the end stream is JSON");
    let error = end.error.expect("the stream ended with an error");
    assert_eq!(error.code, "deadline_exceeded");
    // The limit ends the stream, not the sandbox.
    assert!(h.container.is_running());
}

#[test]
fn an_open_stream_holds_the_sandbox_alive() {
    let h = Harness::new_simulated(123);
    let planner = support::planner(&h, Some(h.id()));
    block_on(planner.sandboxes().create(spec())).expect("create");
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
    let exec_id = "89abcdef01234567";
    let mut response = block_on(planner.handle(control_exec_request(&h.id(), exec_id, false)));
    assert_eq!(response.status, 200);
    let first = block_on(futures::StreamExt::next(&mut response.body))
        .expect("a first chunk")
        .expect("the first chunk is valid");
    assert_ne!(first.len(), 0);

    // Hold the stream for three idle periods. The guard touches the sandbox,
    // so its idle deadline stays ahead of the clock.
    h.clock.advance_wall(Duration::from_mins(30));
    h.clock.advance(Duration::from_mins(30));
    wait_for_idle_deadline(&h, 4 * IDLE_MS);

    // The alarm fires at what was the idle deadline. The sandbox must stay.
    block_on(planner.on_alarm());
    assert!(h.container.is_running(), "the open stream kept the sandbox");
    assert_eq!(planner.last_stop_reason(), None);
    assert_eq!(block_on(planner.sandboxes().list()).expect("list").len(), 1);
    assert_eq!(h.alarm.value(), Some(4 * IDLE_MS));

    // The stream limit still ends the stream, at its own deadline.
    h.clock.advance_wall(Duration::from_secs(1_801));
    h.clock.advance(Duration::from_secs(1_801));
    let mut chunks = Vec::new();
    while let Some(item) = block_on(futures::StreamExt::next(&mut response.body)) {
        chunks.push(item.expect("a later chunk is valid"));
    }
    let last = chunks.pop().expect("the stream ends with one frame");
    let (envelope, _used) =
        Envelope::decode(&Bytes::from(last)).expect("the last chunk is a frame");
    assert!(envelope.is_end_stream(), "the frame is an end stream");
    let end: EndStreamResponse =
        serde_json::from_slice(&envelope.data).expect("the end stream is JSON");
    let error = end.error.expect("the stream ended with an error");
    assert_eq!(error.code, "deadline_exceeded");
    assert!(h.container.is_running(), "the limit ends the stream only");
}

#[test]
fn a_short_stream_ends_without_a_deadline() {
    // A stream that ends before the limit keeps its own end and no error.
    let h = Harness::new_simulated(121);
    let planner = support::planner(&h, Some(h.id()));
    block_on(planner.sandboxes().create(spec())).expect("create");
    h.scripted.with_exec_events(vec![
        ExecEventKind::Started { pid: 1 },
        ExecEventKind::Exit {
            code: Some(0),
            signal: None,
        },
    ]);
    let exec_id = "fedcba9876543210";
    let mut response = block_on(planner.handle(control_exec_request(&h.id(), exec_id, false)));
    assert_eq!(response.status, 200);
    let mut chunks = Vec::new();
    while let Some(item) = block_on(futures::StreamExt::next(&mut response.body)) {
        chunks.push(item.expect("a valid chunk"));
    }
    let last = chunks.pop().expect("the stream has an end frame");
    let (envelope, _used) =
        Envelope::decode(&Bytes::from(last)).expect("the last chunk is a frame");
    assert!(envelope.is_end_stream());
    let end: EndStreamResponse =
        serde_json::from_slice(&envelope.data).expect("the end stream is JSON");
    assert!(end.error.is_none(), "a normal end has no error");
}

#[test]
fn a_host_with_no_timer_keeps_the_stream() {
    // A context with no effect environment has no timer, as the Worker's
    // `Ctx::host()` had before the entry built the bundle. The limit cannot
    // run, so the stream must keep every frame and end on its own.
    let h = Harness::new_simulated(122);
    let planner = support::planner(&h, Some(h.id()));
    block_on(planner.sandboxes().create(spec())).expect("create");
    let no_env = Ctx::with_seed(Arc::clone(&h.clock) as Arc<dyn Clock>, 122);
    let planner = block_on(DoControlPlane::open(
        no_env,
        h.host.clone(),
        None,
        support::limits(),
    ))
    .expect("the second control plane opens")
    .expect("the second control plane has the stored identifier");
    h.scripted.with_exec_events(vec![
        ExecEventKind::Started { pid: 1 },
        ExecEventKind::Exit {
            code: Some(0),
            signal: None,
        },
    ]);
    let exec_id = "0011223344556677";
    let mut response = block_on(planner.handle(control_exec_request(&h.id(), exec_id, false)));
    assert_eq!(response.status, 200);
    let mut chunks = Vec::new();
    while let Some(item) = block_on(futures::StreamExt::next(&mut response.body)) {
        chunks.push(item.expect("a valid chunk"));
    }
    let last = chunks.pop().expect("the stream has an end frame");
    let (envelope, _used) =
        Envelope::decode(&Bytes::from(last)).expect("the last chunk is a frame");
    assert!(envelope.is_end_stream());
    let end: EndStreamResponse =
        serde_json::from_slice(&envelope.data).expect("the end stream is JSON");
    assert!(
        end.error.is_none(),
        "a missing timer adds no deadline error"
    );
}
