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
    ctx.store(crate::outbox::BrowserReply(prepared));
    if crate::install_view_services(&ctx, foundation_ui_runtime::action::Origin::Host).is_err() {
        return 1;
    }
    let Ok(actions) = crate::host_registry() else {
        return 2;
    };
    use foundation_ui_runtime::action::{Origin, Scope};
    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        fn ewe_todo_declare(origin: u32, name: *const u8, len: usize, scope: u32);
    }
    for (name, origin, scope) in actions.declarations() {
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
        // SAFETY: JavaScript reads this UTF-8 slice synchronously during registration.
        unsafe {
            ewe_todo_declare(origin, name.as_ptr(), name.len(), scope);
        }
    }
    LocalApp::builder(ctx, actions).build().install();
    0
}

struct BrowserStorage;

#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn ewe_todo_storage_size() -> i32;
    fn ewe_todo_storage_read(destination: *mut u8, length: usize) -> i32;
    fn ewe_todo_storage_save(contents: *const u8, length: usize) -> i32;
    fn ewe_todo_prepared(
        call: u32,
        contents: *const u8,
        length: usize,
        key: *const u8,
        key_len: usize,
    );
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

fn prepared(call: u32, plan: &crate::outbox::Prepared) {
    let request = plan.request.as_deref().unwrap_or_default();
    let key = plan.key.as_deref().unwrap_or_default();
    // The host copies both slices before it releases this callback.
    unsafe {
        ewe_todo_prepared(
            call,
            request.as_ptr(),
            request.len(),
            key.as_ptr(),
            key.len(),
        );
    }
}
