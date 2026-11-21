//! Browser-local todo action dispatcher.
use foundation_context::{Ctx, MonotonicClock};
use foundation_ui_runtime::action::Origin;
use foundation_ui_runtime::dispatch::{
    Connection, DispatcherBuilder, LocalDispatcher, RenderTarget,
};
use foundation_ui_wire::{writer::ClientCaps, MountSurface};
use std::sync::Arc;

#[no_mangle]
pub extern "C" fn init_todo_host() -> u32 {
    let ctx = Ctx::fixed(Arc::new(MonotonicClock::new()));
    ctx.store(RenderTarget(ui_todo::PAGE_ROOT));
    if ui_todo::install_view_services(&ctx, 1 << 31, u32::MAX).is_err() {
        return 1;
    }
    let Ok(actions) = ui_todo::host_registry() else {
        return 2;
    };
    let dispatcher = DispatcherBuilder::new(Origin::Host, Arc::new(actions)).build();
    let conn = Connection::stateless(ClientCaps::web_current(), MountSurface::Web, 2);
    foundation_core::valtron::single::initialize_pool_with_ctx(0, ctx.clone());
    foundation_wasm_ui::actions::install_local_dispatcher(LocalDispatcher::new(
        dispatcher, ctx, conn,
    ));
    0
}
