//! Browser-local todo action dispatcher.
use foundation_context::{Ctx, MonotonicClock};
use foundation_ui_runtime::dispatch::RenderTarget;
use foundation_wasm_ui::actions::LocalApp;
use std::sync::Arc;

/// Installs the local action dispatcher for the browser.
#[unsafe(no_mangle)]
pub extern "C" fn init_todo_host() -> u32 {
    let ctx = Ctx::fixed(Arc::new(MonotonicClock::new()));
    ctx.store(RenderTarget(crate::PAGE_ROOT));
    if crate::install_view_services(&ctx, 1 << 31, u32::MAX).is_err() {
        return 1;
    }
    let Ok(actions) = crate::host_registry() else {
        return 2;
    };
    LocalApp::builder(ctx, actions).build().install();
    0
}
