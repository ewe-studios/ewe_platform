//! Browser-local todo action dispatcher.
use foundation_context::Ctx;
use foundation_wasm_ui::actions::LocalApp;

/// Install the local action dispatcher for the browser.
///
/// Skip the origin's legacy data when the page uses a different Todo service.
#[unsafe(no_mangle)]
pub extern "C" fn init_todo_host(skip_root_migration: u32) -> u32 {
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
    let migration =
        (skip_root_migration == 0).then(|| crate::shared::pending::records::migrate(ctx.clone()));
    let app = LocalApp::builder(ctx, actions);
    if let Some(migration) = migration {
        app.on_boot(migration).install();
    } else {
        app.install();
    }
    0
}
