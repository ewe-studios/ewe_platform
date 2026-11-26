//! Browser-local todo action dispatcher.
use foundation_context::Ctx;
use foundation_wasm_ui::actions::LocalApp;

/// Installs the local action dispatcher for the browser.
#[unsafe(no_mangle)]
pub extern "C" fn init_todo_host() -> u32 {
    let Ok(ctx) = Ctx::try_host() else {
        return 3;
    };
    let Ok(env) = foundation_core::env::wasm(&ctx) else {
        return 3;
    };
    ctx.store(env);
    crate::install_cache(&ctx);
    if crate::install_view_services(&ctx, foundation_ui_runtime::action::Origin::Host).is_err() {
        return 1;
    }
    let Ok(actions) = crate::host_registry() else {
        return 2;
    };
    let migration = crate::shared::pending::records::migrate(ctx.clone());
    LocalApp::builder(ctx, actions).on_boot(migration).install();
    0
}
