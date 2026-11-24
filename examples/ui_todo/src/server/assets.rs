//! Browser assets for the todo example.
use foundation_http::{
    get,
    native::handlers::{
        static_asset::StaticAssetHandler,
        static_file::{StaticFileConfig, StaticFileHandler},
    },
    shared::router::Router,
    Serve,
};
use std::sync::Arc;

pub(super) fn register(
    router: &mut Router<Arc<dyn Serve>>,
    module: &std::path::Path,
    public: Option<&std::path::Path>,
) -> Result<(), super::Error> {
    register_resources(router, public)?;
    router.at(
        "/module.wasm",
        get(StaticFileHandler::file(module, StaticFileConfig::default())),
    )?;
    if let Some(root) = module.parent() {
        router.at("/mathalley/*", get(StaticFileHandler::new(root)))?;
        router.at("/bindings/*", get(StaticFileHandler::new(root)))?;
        router.at("/rpc/*", get(StaticFileHandler::new(root)))?;
        router.at("/resources-wasm/*", get(StaticFileHandler::new(root)))?;
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
        ("/game-view.js", "text/javascript", include_bytes!("../../web/game-view.js").as_slice()),
        (
            "/foundation-ui-runtime.js",
            "text/javascript",
            include_bytes!(concat!(env!("OUT_DIR"), "/foundation-ui-runtime.js")).as_slice(),
        ),
        (
            "/foundation-ui-runtime.min.js",
            "text/javascript",
            include_bytes!(concat!(env!("OUT_DIR"), "/foundation-ui-runtime.min.js")).as_slice(),
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
            "/foundation-ui-recovery.js",
            "text/javascript",
            include_bytes!(
                "../../../../backends/foundation_wasm_ui/runtimes/foundation-ui-recovery.js"
            )
            .as_slice(),
        ),
        (
            "/foundation-ui-connections.js",
            "text/javascript",
            include_bytes!(
                "../../../../backends/foundation_wasm_ui/runtimes/foundation-ui-connections.js"
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

#[cfg(not(debug_assertions))]
#[derive(foundation_macros::EmbedDirectoryAs)]
#[source = "$EWE_STAGED_RESOURCES/."]
#[always_embed]
struct PackagedResources;

fn register_resources(
    router: &mut Router<Arc<dyn Serve>>,
    public: Option<&std::path::Path>,
) -> Result<(), super::Error> {
    #[cfg(debug_assertions)]
    let directory = match public {
        Some(path) => path.to_owned(),
        None => foundation_resources::server::sibling_public(env!("CARGO_PKG_NAME"))?,
    };
    #[cfg(debug_assertions)]
    let public = Some(directory.as_path());
    #[cfg(debug_assertions)]
    let embedded = |_: &str| None;
    #[cfg(not(debug_assertions))]
    let embedded = |name: &str| {
        use foundation_nostd::embeddable::DirectoryData;
        PackagedResources.read_utf8_for(name)
    };
    foundation_resources::server::register(
        router,
        include_str!(env!("EWE_RESOURCE_NAMES")),
        public,
        None,
        embedded,
    )
}
