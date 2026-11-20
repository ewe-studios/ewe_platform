//! Build the native todo core with the same host actions and views as the browser.
use foundation_context::Ctx;
use foundation_ui_native::{CoreConfig, CoreError, HostTransport, NativeCore, NativePlatform};
use foundation_ui_runtime::{
    action::{Origin, Scope},
    dispatch::RenderTarget,
};
use foundation_ui_wire::writer::WriterConfig;
use std::sync::Arc;
uniffi::setup_scaffolding!();

/// Installs the presentation signer, local todo actions, and remote server declarations.
///
/// # Errors
/// Returns a setup error when an action declaration or signer is invalid.
#[uniffi::export]
pub fn create_todo_core(
    platform: NativePlatform,
    transport: Arc<dyn HostTransport>,
) -> Result<Arc<NativeCore>, CoreError> {
    let ctx = Ctx::host();
    ui_todo::install_view_services(&ctx, 1 << 31, u32::MAX).map_err(|e| CoreError::Write {
        detail: e.to_string(),
    })?;
    ctx.store(RenderTarget(ui_todo::PAGE_ROOT));
    let mut registry = ui_todo::host_registry().map_err(|e| CoreError::BadReference {
        detail: e.to_string(),
    })?;
    for name in ["add_todo", "toggle_todo", "confirm_delete"] {
        registry
            .declare(Some(Origin::Server), name, Scope::All)
            .map_err(|e| CoreError::BadReference {
                detail: e.to_string(),
            })?;
    }
    Ok(NativeCore::new(
        CoreConfig {
            platform,
            ctx,
            registry: Arc::new(registry),
            writer: WriterConfig::default(),
        },
        Some(transport),
    ))
}
