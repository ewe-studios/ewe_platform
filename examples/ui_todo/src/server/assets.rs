//! Register the Todo theme and staged application resources.
use foundation_http::{
    get,
    native::handlers::{static_asset::StaticAssetHandler, static_file::StaticFileHandler},
    shared::router::Router,
    Serve,
};
use std::{path::Path, sync::Arc};

foundation_resources::packaged_resources!(PackagedResources);

pub(super) fn register(
    router: &mut Router<Arc<dyn Serve>>,
    public: Option<&Path>,
) -> Result<(), super::Error> {
    router.at(
        "/theme.css",
        get(StaticAssetHandler::new(
            crate::todo_theme().to_css().as_bytes(),
            "text/css; charset=utf-8",
        )),
    )?;
    match public {
        Some(directory) => router.mount(
            StaticFileHandler::from_resources(PackagedResources).with_public_directory(directory),
        )?,
        None => router.mount(StaticFileHandler::from_resources(PackagedResources))?,
    }
    Ok(())
}
