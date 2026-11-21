//! Browser assets for the todo example.
use foundation_http::{
    get,
    native::handlers::{
        static_asset::StaticAssetHandler,
        static_file::{StaticFileConfig, StaticFileHandler},
    },
    shared::router::{RouteResult, Router},
    Serve,
};
use std::sync::Arc;

pub(super) fn register(
    router: &mut Router<Arc<dyn Serve>>,
    module: &std::path::Path,
) -> RouteResult<()> {
    router.at(
        "/module.wasm",
        get(StaticFileHandler::file(module, StaticFileConfig::default())),
    )?;
    if let Some(root) = module.parent() {
        router.at("/rpc/*", get(StaticFileHandler::new(root)))?;
    }
    router.at(
        "/theme.css",
        get(StaticAssetHandler::new(
            crate::todo_theme().to_css().as_bytes(),
            "text/css; charset=utf-8",
        )),
    )?;
    for (url, kind, bytes) in [
        (
            "/",
            "text/html; charset=utf-8",
            include_bytes!("../../web/index.html").as_slice(),
        ),
        (
            "/app.js",
            "text/javascript",
            include_bytes!("../../web/app.js").as_slice(),
        ),
        (
            "/foundation-wasm.js",
            "text/javascript",
            include_bytes!("../../../../backends/foundation_wasm/runtime/foundation-wasm.js")
                .as_slice(),
        ),
        (
            "/foundation-wasm-ui.js",
            "text/javascript",
            include_bytes!(
                "../../../../backends/foundation_wasm_ui/runtimes/foundation-wasm-ui.js"
            )
            .as_slice(),
        ),
        (
            "/foundation-ui-actions.js",
            "text/javascript",
            include_bytes!(
                "../../../../backends/foundation_wasm_ui/runtimes/foundation-ui-actions.js"
            )
            .as_slice(),
        ),
        (
            "/foundation-ui-frames.js",
            "text/javascript",
            include_bytes!(
                "../../../../backends/foundation_wasm_ui/runtimes/foundation-ui-frames.js"
            )
            .as_slice(),
        ),
        (
            "/foundation-ui-live.js",
            "text/javascript",
            include_bytes!(
                "../../../../backends/foundation_wasm_ui/runtimes/foundation-ui-live.js"
            )
            .as_slice(),
        ),
        (
            "/foundation-ui-behaviours.js",
            "text/javascript",
            include_bytes!(
                "../../../../backends/foundation_wasm_ui/runtimes/foundation-ui-behaviours.js"
            )
            .as_slice(),
        ),
        (
            "/foundation-ui-vocab.js",
            "text/javascript",
            include_bytes!(
                "../../../../backends/foundation_wasm_ui/runtimes/foundation-ui-vocab.js"
            )
            .as_slice(),
        ),
        (
            "/foundation-ui-wire.js",
            "text/javascript",
            include_bytes!(
                "../../../../backends/foundation_wasm_ui/runtimes/foundation-ui-wire.js"
            )
            .as_slice(),
        ),
    ] {
        router.at(url, get(StaticAssetHandler::new(bytes, kind)))?;
    }
    Ok(())
}
