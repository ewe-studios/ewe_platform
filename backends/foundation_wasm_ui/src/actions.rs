//! Run web actions on the browser thread. Each call owns one future and one reply.

extern crate std;

use alloc::{collections::BTreeMap, rc::Rc, sync::Arc, vec::Vec};
use core::{
    cell::RefCell,
    future::poll_fn,
    task::{Context, Poll, Waker},
};
use foundation_core::valtron::{single, FutureTask};
use foundation_ui_runtime::action::TransportFuture;
use foundation_wasm::{internal_api, MemoryId};

/// The browser receives this value when a call ends without a reply allocation.
pub const MISSING_REPLY: u64 = u64::MAX;

type Start = dyn Fn(&[u8]) -> TransportFuture<'static>;
type Complete = dyn Fn(u64, u64);

struct Call {
    generation: Arc<()>,
    work: Option<TransportFuture<'static>>,
    wake: Option<Waker>,
    complete: Rc<Complete>,
}

#[derive(Default)]
struct Bridge {
    host: Option<(Rc<Start>, Rc<Complete>)>,
    calls: BTreeMap<u64, Call>,
}

std::thread_local! { static BRIDGE: RefCell<Bridge> = RefCell::default(); }

/// Install the browser dispatcher after the host initializes the thread's Valtron pool.
#[cfg(target_family = "wasm")]
pub fn install_local_dispatcher(local: foundation_ui_runtime::dispatch::LocalDispatcher) {
    let local = Arc::new(local);
    install_action_bridge(
        move |bytes| local.clone().submit_owned(bytes),
        |call, slot| {
            #[link(wasm_import_module = "env")]
            extern "C" {
                fn ewe_ui_action_done(call_id: u64, reply_slot: u64);
            }
            // The browser copies the bytes and releases every valid reply slot.
            unsafe { ewe_ui_action_done(call, slot) };
        },
    );
}

/// Install the request and completion functions for an embedded host.
/// The completion function owns each valid reply slot.
pub fn install_action_bridge(
    start: impl Fn(&[u8]) -> TransportFuture<'static> + 'static,
    complete: impl Fn(u64, u64) + 'static,
) {
    uninstall_local_dispatcher();
    BRIDGE.with_borrow_mut(|bridge| bridge.host = Some((Rc::new(start), Rc::new(complete))));
}

/// Drop pending actions and signal one failed completion for each caller.
pub fn uninstall_local_dispatcher() {
    let calls = BRIDGE.with_borrow_mut(|bridge| {
        bridge.host = None;
        core::mem::take(&mut bridge.calls)
    });
    for (id, call) in calls {
        let Call {
            complete,
            wake,
            work,
            ..
        } = call;
        drop(work);
        if let Some(waker) = wake {
            waker.wake();
        }
        complete(id, MISSING_REPLY);
    }
}

/// Release the installed bridge and its pending calls.
#[no_mangle]
pub extern "C" fn ewe_ui_actions_close() {
    uninstall_local_dispatcher();
}

fn take_request(raw: u64) -> Option<Vec<u8>> {
    internal_api::with_global_allocations(|arena| {
        let id = MemoryId::from_u64(raw);
        let bytes = arena.get(id).ok()?.take();
        arena.deallocate(id).ok()?;
        bytes
    })
}

fn reply_slot(bytes: Vec<u8>) -> Option<u64> {
    internal_api::with_global_allocations(|arena| {
        let id = arena.allocate(0).ok()?;
        if let Some(()) = arena
            .get(id)
            .ok()
            .and_then(|slot| slot.into_with(|buffer| *buffer = bytes))
        {
            return Some(id.as_u64());
        }
        arena.deallocate(id).ok()?;
        None
    })
}

fn matching_call<'a>(
    bridge: &'a mut Bridge,
    id: u64,
    generation: &Arc<()>,
) -> Option<&'a mut Call> {
    bridge
        .calls
        .get_mut(&id)
        .filter(|call| Arc::ptr_eq(&call.generation, generation))
}

fn complete_call(id: u64, generation: &Arc<()>, bytes: Option<Vec<u8>>) {
    let call = BRIDGE.with_borrow_mut(|bridge| {
        matching_call(bridge, id, generation)?;
        bridge.calls.remove(&id)
    });
    if let Some(call) = call {
        let slot = bytes.and_then(reply_slot).unwrap_or(MISSING_REPLY);
        (call.complete)(id, slot);
    }
}

fn poll_call(id: u64, generation: &Arc<()>, ctx: &mut Context<'_>) -> Poll<()> {
    let work = BRIDGE.with_borrow_mut(|bridge| {
        let call = matching_call(bridge, id, generation)?;
        call.wake = Some(ctx.waker().clone());
        call.work.take()
    });
    let Some(mut work) = work else {
        return Poll::Ready(());
    };
    match work.as_mut().poll(ctx) {
        Poll::Ready(reply) => {
            complete_call(id, generation, reply.ok().flatten());
            Poll::Ready(())
        }
        Poll::Pending => BRIDGE.with_borrow_mut(|bridge| {
            if let Some(call) = matching_call(bridge, id, generation) {
                call.work = Some(work);
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        }),
    }
}

/// Start a local action. Rust releases the request slot on every path.
/// Status zero means accepted. Other statuses identify an invalid slot, missing host,
/// missing executor, duplicate call or scheduling failure, in that order.
#[no_mangle]
pub extern "C" fn ewe_ui_action(id: u64, slot: u64) -> u32 {
    let Some(bytes) = take_request(slot) else {
        return 1;
    };
    let Some((start, complete)) = BRIDGE.with_borrow(|bridge| bridge.host.clone()) else {
        return 2;
    };
    if !single::is_initialized() {
        return 3;
    }
    if BRIDGE.with_borrow(|bridge| bridge.calls.contains_key(&id)) {
        return 4;
    }
    let generation = Arc::new(());
    let call = Call {
        generation: generation.clone(),
        work: None,
        wake: None,
        complete,
    };
    BRIDGE.with_borrow_mut(|bridge| bridge.calls.insert(id, call));
    // Admission claims the sequence before any scheduled task can poll.
    let work = start(&bytes);
    let active = BRIDGE.with_borrow_mut(|bridge| {
        matching_call(bridge, id, &generation).map(|call| call.work = Some(work))
    });
    if active.is_none() {
        return 0;
    } // A reentrant close already settled this caller.
    let on_panic = generation.clone();
    let task = FutureTask::new(poll_fn(move |ctx| poll_call(id, &generation, ctx)));
    if single::spawn()
        .with_task(task)
        .with_panic_handler(move |_| complete_call(id, &on_panic, None))
        .as_scheduled()
        .spawn()
        .is_err()
    {
        BRIDGE.with_borrow_mut(|bridge| bridge.calls.remove(&id));
        return 5;
    }
    #[cfg(target_family = "wasm")]
    foundation_core::valtron::wasm::request_resume();
    0
}
