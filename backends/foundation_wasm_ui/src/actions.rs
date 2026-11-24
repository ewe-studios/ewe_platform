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

type Start = dyn Fn(u64, &[u8]) -> TransportFuture<'static>;
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

/// Owns the action dispatcher for one browser application.
pub struct LocalApp(foundation_ui_runtime::dispatch::LocalDispatcher);

/// Keeps the application context and actions until startup.
pub struct LocalAppBuilder {
    ctx: foundation_core::valtron::Ctx,
    actions: foundation_ui_runtime::action::ActionRegistry,
    #[cfg(all(feature = "browser-storage", target_family = "wasm"))]
    startup: Option<crate::storage::Startup>,
}

impl LocalApp {
    #[must_use]
    pub fn builder(
        ctx: foundation_core::valtron::Ctx,
        actions: foundation_ui_runtime::action::ActionRegistry,
    ) -> LocalAppBuilder {
        LocalAppBuilder {
            ctx,
            actions,
            #[cfg(all(feature = "browser-storage", target_family = "wasm"))]
            startup: None,
        }
    }

    /// Starts the browser task pool and installs the action dispatcher.
    #[cfg(target_family = "wasm")]
    pub fn install(self) {
        single::initialize_pool_with_ctx(0, self.0.context().clone());
        install_local_dispatcher(self.0);
    }

    /// Install browser child calls. The host supplies the imports from `WasmActionTransport`.
    #[cfg(target_family = "wasm")]
    pub fn install_with_child_actions(self) {
        single::initialize_pool_with_ctx(0, self.0.context().clone());
        let local = Arc::new(self.0);
        install_bridge(
            move |id, bytes| {
                local
                    .clone()
                    .submit_with_caller(bytes, Some(Arc::new(child::BrowserCaller(id))))
            },
            notify_completion,
        );
    }

    /// Gives a custom host the prepared dispatcher.
    #[must_use]
    pub fn into_dispatcher(self) -> foundation_ui_runtime::dispatch::LocalDispatcher {
        self.0
    }
}

impl LocalAppBuilder {
    /// Run application setup after service installation and before the first view opens.
    #[cfg(all(feature = "browser-storage", target_family = "wasm"))]
    #[must_use]
    pub fn on_boot(
        mut self,
        task: impl core::future::Future<Output = Result<(), alloc::string::String>> + 'static,
    ) -> Self {
        self.startup = Some(alloc::boxed::Box::pin(task));
        self
    }

    /// Publish this registry to the browser and start the local action dispatcher.
    #[cfg(target_family = "wasm")]
    pub fn install(self) {
        use foundation_ui_runtime::action::{Origin, Scope};
        #[link(wasm_import_module = "env")]
        extern "C" {
            fn ewe_ui_declare(origin: u32, name: *const u8, length: usize, scope: u32);
        }
        #[cfg(feature = "browser-storage")]
        let builder = {
            let mut builder = self;
            crate::storage::install(&builder.ctx, builder.startup.take());
            builder
        };
        #[cfg(not(feature = "browser-storage"))]
        let builder = self;
        for (name, origin, scope) in builder.actions.declarations() {
            let origin = match origin {
                None => 0,
                Some(Origin::Host) => 1,
                Some(Origin::Server) => 2,
            };
            let scope = match scope {
                Scope::All => 0,
                Scope::Web => 1,
                Scope::Mobile => 2,
            };
            // The browser reads the name before this call returns.
            unsafe { ewe_ui_declare(origin, name.as_ptr(), name.len(), scope) };
        }
        builder.build().install_with_child_actions();
    }

    #[must_use]
    pub fn build(self) -> LocalApp {
        use foundation_ui_runtime::{
            action::Origin,
            dispatch::{Connection, DispatcherBuilder, LocalDispatcher},
        };
        use foundation_ui_wire::{writer::ClientCaps, MountSurface};
        let dispatcher = DispatcherBuilder::new(Origin::Host, Arc::new(self.actions)).build();
        let conn = Connection::stateless(ClientCaps::web_current(), MountSurface::Web, 2);
        LocalApp(LocalDispatcher::new(dispatcher, self.ctx, conn))
    }
}

/// Install the browser dispatcher after the host initializes the thread's Valtron pool.
#[cfg(target_family = "wasm")]
pub fn install_local_dispatcher(local: foundation_ui_runtime::dispatch::LocalDispatcher) {
    let local = Arc::new(local);
    install_action_bridge(
        move |bytes| local.clone().submit_owned(bytes),
        notify_completion,
    );
}

#[cfg(target_family = "wasm")]
fn notify_completion(call: u64, slot: u64) {
    #[link(wasm_import_module = "env")]
    extern "C" {
        fn ewe_ui_action_done(call_id: u64, reply_slot: u64);
    }
    // The browser copies the bytes and releases the reply slot.
    unsafe { ewe_ui_action_done(call, slot) };
}

#[cfg(target_family = "wasm")]
mod child;

/// Install the request and completion functions for an embedded host.
/// The completion function owns each valid reply slot.
pub fn install_action_bridge(
    start: impl Fn(&[u8]) -> TransportFuture<'static> + 'static,
    complete: impl Fn(u64, u64) + 'static,
) {
    install_bridge(move |_, bytes| start(bytes), complete);
}

fn install_bridge(
    start: impl Fn(u64, &[u8]) -> TransportFuture<'static> + 'static,
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
    let work = start(id, &bytes);
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
