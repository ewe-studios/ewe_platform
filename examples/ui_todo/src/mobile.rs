//! Build the native todo core with the same host actions and views as the browser.
use foundation_context::Ctx;
use foundation_ui_native::{CoreError, EventInput, HostTransport, NativeCore, NativePlatform};
use foundation_ui_runtime::{
    action::{Origin, Scope},
    dispatch::RenderTarget,
};
use std::sync::Arc;

/// Returns the optional native theme from the shared Rust application.
#[uniffi::export]
pub fn todo_theme_manifest() -> String {
    crate::todo_theme().native_manifest()
}

/// Installs the presentation signer, local todo actions, and remote server declarations.
///
/// # Errors
/// Returns a setup error when an action declaration or signer is invalid.
#[uniffi::export]
pub fn create_todo_core(
    platform: NativePlatform,
    transport: Arc<dyn HostTransport>,
) -> Result<Arc<NativeCore>, CoreError> {
    let ctx = todo_context()?;
    let mut registry = crate::host_registry().map_err(|e| CoreError::BadReference {
        detail: e.to_string(),
    })?;
    for name in ["add_todo", "toggle_todo", "confirm_delete"] {
        registry
            .declare(Some(Origin::Server), name, Scope::All)
            .map_err(|e| CoreError::BadReference {
                detail: e.to_string(),
            })?;
    }
    Ok(NativeCore::builder(platform, ctx, registry)
        .with_host_transport(transport)
        .build())
}

/// Creates an on-device core and its task store.
///
/// # Errors
/// Returns an error if the signer or action declarations are invalid.
#[uniffi::export]
pub fn create_local_todo_core(platform: NativePlatform) -> Result<Arc<NativeCore>, CoreError> {
    let ctx = todo_context()?;
    let registry = crate::local_registry(&ctx).map_err(|e| CoreError::BadReference {
        detail: e.to_string(),
    })?;
    Ok(NativeCore::builder(platform, ctx, registry).build())
}

/// Returns the first frame for an on-device screen.
///
/// # Errors
/// Returns an error if the core cannot render the first frame.
#[uniffi::export]
pub async fn initial_todo_frame(core: Arc<NativeCore>) -> Result<Vec<u8>, CoreError> {
    core.trigger(
        "host:initialize_todos".into(),
        EventInput {
            node_id: crate::PAGE_ROOT,
            event_name: String::new(),
            value: String::new(),
            checked: false,
            parent_node: 0,
            state: Vec::new(),
        },
    )
    .await?
    .ok_or_else(|| CoreError::Write {
        detail: "Missing initial todo frame".into(),
    })
}

fn todo_context() -> Result<Ctx, CoreError> {
    let ctx = Ctx::host();
    crate::install_view_services(&ctx, 1 << 31, u32::MAX).map_err(|e| CoreError::Write {
        detail: e.to_string(),
    })?;
    ctx.store(RenderTarget(crate::PAGE_ROOT));
    Ok(ctx)
}

/// A todo returned by the example service.
#[derive(uniffi::Record)]
pub struct TodoResult {
    pub id: u64,
    pub title: String,
}

#[derive(uniffi::Error, Debug, thiserror::Error)]
pub enum TodoServiceError {
    #[error("Todo not found")]
    NotFound,
    #[error("Todo service failed: {detail}")]
    Failed { detail: String },
}

/// Read typed data with the application's native HTTP client.
///
/// # Errors
/// Returns the service status when the item is absent or a request fails.
#[uniffi::export]
pub async fn get_todo(
    client: Arc<foundation_ui_native::rpc::NativeHttpClient>,
    base_url: String,
    id: u64,
) -> Result<TodoResult, TodoServiceError> {
    use foundation_connectrpc::{ClientOptions, Request, RpcCtx};
    let failed = |e: foundation_errstacks::ErrorTrace<foundation_connectrpc::ConnectError>| {
        if e.current_context().code() == foundation_connectrpc::Code::NotFound {
            TodoServiceError::NotFound
        } else {
            TodoServiceError::Failed {
                detail: e.to_string(),
            }
        }
    };
    let transport = Arc::new(foundation_ui_connectrpc::PlatformHttpTransport::new(client));
    let todos = crate::todo_rpc::TodosClient::new(transport, &base_url, ClientOptions::new())
        .map_err(failed)?;
    let reply = todos
        .get_todo(
            RpcCtx::client(Ctx::host()),
            Request::new(crate::todo_rpc::GetTodoRequest {
                id,
                ..Default::default()
            }),
        )
        .await
        .map_err(failed)?
        .msg;
    Ok(TodoResult {
        id: reply.id,
        title: reply.title,
    })
}
