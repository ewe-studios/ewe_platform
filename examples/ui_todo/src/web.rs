//! Browser-local todo action dispatcher.
use foundation_context::Ctx;
use foundation_wasm_ui::actions::LocalApp;
use std::sync::Arc;

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
    crate::install_pending_storage(&ctx, Arc::new(BrowserStorage));
    if crate::install_view_services(&ctx, foundation_ui_runtime::action::Origin::Host).is_err() {
        return 1;
    }
    let Ok(actions) = crate::host_registry() else {
        return 2;
    };
    LocalApp::builder(ctx, actions).install();
    0
}

struct BrowserStorage;

#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn ewe_todo_storage_size() -> i32;
    fn ewe_todo_storage_read(destination: *mut u8, length: usize) -> i32;
    fn ewe_todo_storage_save(contents: *const u8, length: usize) -> i32;

}

#[async_trait::async_trait]
impl crate::PendingStorage for BrowserStorage {
    async fn load(&self) -> Result<Vec<u8>, String> {
        // The host reports storage errors with a negative length.
        let size = usize::try_from(unsafe { ewe_todo_storage_size() })
            .map_err(|_| "Cannot read pending Todos")?;
        let mut bytes = vec![0; size];
        // The initialized buffer is valid for this synchronous host copy.
        if unsafe { ewe_todo_storage_read(bytes.as_mut_ptr(), bytes.len()) } != 0 {
            return Err("Cannot read pending Todos".into());
        }
        Ok(bytes)
    }

    async fn save(&self, bytes: Vec<u8>) -> Result<(), String> {
        // The host copies this slice before returning.
        if unsafe { ewe_todo_storage_save(bytes.as_ptr(), bytes.len()) } != 0 {
            return Err("Cannot save pending Todos".into());
        }
        Ok(())
    }
}
