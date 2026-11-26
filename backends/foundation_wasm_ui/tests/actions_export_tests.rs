//! Test local action slot ownership and Valtron wake delivery.

#![cfg(not(target_family = "wasm"))]

use foundation_core::valtron::single;
use foundation_ui_runtime::action::{
    ActionRegistry, ActionResult, Context, Interaction, Origin, Surface,
};
use foundation_ui_runtime::dispatch::{Connection, DispatcherBuilder, LocalDispatcher};
use foundation_ui_wire::{read_frame, writer::ClientCaps, WireLimits};
use foundation_wasm::{internal_api, MemoryId};
use foundation_wasm_ui::actions::{
    ewe_ui_action, ewe_ui_actions_close, install_action_bridge, MISSING_REPLY,
};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    future::poll_fn,
    rc::Rc,
    sync::{Arc, Mutex},
    task::{Poll, Waker},
};

fn slot(bytes: &[u8]) -> u64 {
    internal_api::with_global_allocations(|arena| {
        let id = arena.allocate(0).unwrap();
        arena
            .get(id)
            .unwrap()
            .apply(|buf| buf.extend_from_slice(bytes));
        id.as_u64()
    })
}

fn exists(id: u64) -> bool {
    internal_api::with_global_allocations(|arena| arena.get(MemoryId::from_u64(id)).is_ok())
}

fn take(id: u64) -> Vec<u8> {
    internal_api::with_global_allocations(|arena| {
        let id = MemoryId::from_u64(id);
        let bytes = arena.get(id).unwrap().take().unwrap();
        arena.deallocate(id).unwrap();
        bytes
    })
}

// Stop when the executor parks. Wakes start the next turn.
fn run_ready() {
    for _ in 0..32 {
        let _progress = single::run_once();
        if !single::has_runnable_work() {
            return;
        }
    }
    panic!("The local executor did not park");
}

#[test]
fn missing_dispatcher_rejects_start() {
    let request = slot(b"request");
    assert_eq!(ewe_ui_action(1, request), 2);
    assert!(!exists(request));
    let next = slot(b"new owner");
    assert_eq!(ewe_ui_action(1, request), 1);
    assert_eq!(take(next), b"new owner");
}

#[test]
fn missing_executor_rejects_start() {
    install_action_bridge(|_| panic!("started without executor"), |_, _| panic!());
    let request = slot(b"request");
    assert_eq!(ewe_ui_action(5, request), 3);
    assert!(!exists(request));
    ewe_ui_actions_close();
}

#[foundation_ui_runtime::action(host)]
async fn press(_ctx: &Context, _event: Interaction<()>) -> ActionResult {
    Ok(())
}

#[test]
fn ready_action_returns_frame() {
    use foundation_ui_wire::{
        finish_frame, ActionRequestT, FrameKindT, FrameRequestT, FrameT, SCHEMA_VERSION,
    };
    single::initialize_pool(4);
    let mut registry = ActionRegistry::new();
    registry.register(press).unwrap();
    let dispatcher = DispatcherBuilder::new(Origin::Host, Arc::new(registry)).build();
    let conn = Connection::stateless(ClientCaps::web_current(), Surface::Web, 42);
    let local = Arc::new(LocalDispatcher::new(dispatcher, single::fork_ctx(), conn));
    let replies = Rc::new(RefCell::new(Vec::new()));
    let received = replies.clone();
    install_action_bridge(
        move |bytes| local.clone().submit_owned(bytes),
        move |id, slot| {
            received.borrow_mut().push((id, take(slot)));
        },
    );

    let mut action = ActionRequestT::default();
    action.action = Some("host:press".into());
    action.node_id = 9;
    let mut body = FrameRequestT::default();
    body.action = Some(Box::new(action));
    let mut frame = FrameT::default();
    frame.schema_version = SCHEMA_VERSION;
    frame.stream_id = 42;
    frame.sequence_id = 3;
    frame.kind = FrameKindT::FrameRequest(Box::new(body));
    let request = slot(&finish_frame(&frame).unwrap());
    assert_eq!(ewe_ui_action(7, request), 0);
    assert!(!exists(request));
    run_ready();
    let replies = replies.borrow();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].0, 7);
    let frame = read_frame(&replies[0].1, &WireLimits::default()).unwrap();
    assert_eq!(frame.stream_id(), 42);
    let body = frame.kind_as_frame_message().unwrap();
    assert_eq!(body.ack(), 3);
    assert!(body.messages().is_none_or(|messages| messages.is_empty()));
    ewe_ui_actions_close();
}

#[derive(Default)]
struct Gate {
    open: bool,
    wake: Option<Waker>,
}

#[test]
fn pending_action_wakes_and_returns_frame() {
    single::initialize_pool(5);
    let gates = Arc::new(Mutex::new(BTreeMap::<u8, Gate>::new()));
    let calls = gates.clone();
    let replies = Rc::new(RefCell::new(Vec::new()));
    let received = replies.clone();
    install_action_bridge(
        move |bytes| {
            let key = bytes[0];
            let calls = calls.clone();
            Box::pin(poll_fn(move |ctx| {
                let mut map = calls.lock().unwrap();
                let gate = map.entry(key).or_default();
                if gate.open {
                    return Poll::Ready(Ok(Some(vec![key])));
                }
                gate.wake = Some(ctx.waker().clone());
                Poll::Pending
            }))
        },
        move |id, slot| {
            received.borrow_mut().push((
                id,
                if slot == MISSING_REPLY {
                    None
                } else {
                    Some(take(slot))
                },
            ));
        },
    );
    for id in 1..=3_u8 {
        let request = slot(&[id]);
        assert_eq!(ewe_ui_action(u64::from(id), request), 0);
        assert!(!exists(request));
    }
    let duplicate = slot(&[9]);
    assert_eq!(ewe_ui_action(1, duplicate), 4);
    assert!(!exists(duplicate));
    run_ready();
    assert!(replies.borrow().is_empty());
    for key in [2, 1] {
        let wake = {
            let mut map = gates.lock().unwrap();
            let gate = map.get_mut(&key).unwrap();
            gate.open = true;
            gate.wake.take().unwrap()
        };
        wake.wake();
        run_ready();
    }
    assert_eq!(*replies.borrow(), [(2, Some(vec![2])), (1, Some(vec![1]))]);
    ewe_ui_actions_close();
    gates
        .lock()
        .unwrap()
        .get_mut(&3)
        .unwrap()
        .wake
        .take()
        .unwrap()
        .wake();
    run_ready();
    assert_eq!(replies.borrow().last(), Some(&(3, None)));
    assert_eq!(replies.borrow().len(), 3);
}
